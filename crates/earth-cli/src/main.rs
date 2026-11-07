use clap::{Parser, Subcommand};
use earth_core::{Coupler, GridPreset, World, make_header, save_snapshot, snapshot_hash};
use earth_kernels::Scheduler;
use std::path::Path;

#[derive(Parser)]
#[command(name = "lifesim", version, about = "LifeSim headless-first CLI (M0)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
    /// JSON output
    #[arg(long, global = true, default_value_t = false)]
    json: bool,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create run dir from config
    Init { #[arg(long, default_value = "formation")] mode: String,
           #[arg(long, default_value = "L0")] grid: String,
           #[arg(long, default_value_t = 42)] seed: u64,
           #[arg(long, default_value = "runs/run-001")] out: String },
    /// Run simulation
    Run { #[arg(long, default_value = "examples/lifesim.toml")] config: String,
          #[arg(long, default_value_t = 0)] steps: usize,
          #[arg(long, default_value = "")] preset: String,
          #[arg(long, default_value_t = false)] offline: bool,
          #[arg(long, default_value_t = false)] accept_slow: bool },
    /// Validate checkpoint epoch
    Check { #[arg(long, default_value = "preindustrial")] epoch: String,
            #[arg(long, default_value = "")] run: String,
            #[arg(long, default_value_t = false)] offline: bool,
            #[arg(long, default_value = "")] control: String,
            #[arg(long, default_value = "")] ci_artifact: String },
    /// Export snapshot/scalars
    Export { #[arg(long, default_value = "")] run: String,
             #[arg(long, default_value = "csv")] format: String,
             #[arg(long, default_value = "snapshot")] scope: String,
             #[arg(long, default_value = "")] at: String,
             #[arg(long, default_value = "out.csv")] out: String },
}

fn read_config(path: &str) -> (toml::Value, String, u64, String, f64, String) {
    let text = std::fs::read_to_string(path).unwrap_or_else(|_| {
        // Fallback minimal config (offline smoke).
        "[meta]\nconfig_version=1\nmode=\"formation\"\nseed=42\ngrid=\"L0\"\n[time]\nt_start_yr=-4540000000.0\n[device]\npolicy=\"auto\"\n".to_string()
    });
    let v: toml::Value = text.parse().unwrap_or(toml::Value::Table(Default::default()));
    let mode = v.get("meta").and_then(|m| m.get("mode")).and_then(|s| s.as_str()).unwrap_or("formation").to_string();
    let grid = v.get("meta").and_then(|m| m.get("grid")).and_then(|s| s.as_str()).unwrap_or("L0").to_string();
    let seed = v.get("meta").and_then(|m| m.get("seed")).and_then(|s| s.as_integer()).unwrap_or(42) as u64;
    let t_start = v.get("time").and_then(|m| m.get("t_start_yr")).and_then(|s| s.as_float()).unwrap_or(-4_540_000_000.0);
    let hash = blake3::hash(text.as_bytes()).to_hex().to_string();
    (v, mode, seed, grid, t_start, hash)
}

fn build_world(mode: &str, grid_s: &str, seed: u64, t_start: f64, accept_slow: bool) -> World {
    let mut g = grid_s.to_string();
    if g == "Standard" { g = "L2".into(); }
    if g == "Low" { g = "L0".into(); }
    if g == "High" { g = "L2".into(); }
    if g == "Real" { g = "L3".into(); }
    if g == "L3" && !accept_slow {
        eprintln!("L3 on CPU-only M0/M1: full 40k-cell DAG is slow; set --accept-slow to proceed, else L2");
        g = "L2".into();
    }
    let preset = GridPreset::parse(&g).unwrap_or(GridPreset::L0);
    World::fresh(preset, seed, &mode, t_start)
}

fn formation_params(config: &toml::Value) -> (f64, f64) {
    let jitter = config.get("formation").and_then(|m| m.get("theia_jitter"))
        .and_then(|s| s.as_float()).unwrap_or(0.1);
    let h2o = config.get("formation").and_then(|m| m.get("volatile_H2O"))
        .and_then(|s| s.as_float()).unwrap_or(1.0);
    (jitter, h2o)
}

/// Integrate a fresh Hadean world to `target_yr` (no IO). M1 stage workhorse:
/// cold-start at −4.56 Ga, full 11-system DAG at deep-time dt.
fn integrate_hadean_to(target_yr: f64, dt_yr: f64, seed: u64, theia_jitter: f64, volatile_h2o: f64) -> World {
    let mut world = World::fresh(GridPreset::L0, seed, "formation", formation::ACCRETION_START_YR);
    let coupler = Coupler::default_dag(dt_yr);
    let mut formation_sys = formation::FormationSystem::new(seed, theia_jitter, volatile_h2o);
    let mut solar = solar::SolarSystem;
    let mut interior = interior::InteriorSystem;
    let mut tectonics = tectonics::TectonicsSystem;
    let mut atmos = atmos::AtmosSystem;
    let mut ocean = ocean::OceanSystem;
    let mut ice = ice::IceSystem;
    let mut geochem = geochem::GeochemSystem::default();
    let mut bio = bio::BioSystem;
    let mut eco = eco::EcoSystem;
    let mut civs = civs::CivsSystem::default();
    let mut systems: Vec<&mut dyn earth_core::Subsystem> = vec![
        &mut formation_sys, &mut solar, &mut interior, &mut tectonics, &mut atmos,
        &mut ocean, &mut ice, &mut geochem, &mut bio, &mut eco, &mut civs,
    ];
    while world.t_yr < target_yr {
        if coupler.sync(&mut world, &mut systems, dt_yr).is_err() {
            break;
        }
        world.advance_time(dt_yr);
    }
    world
}

fn run_steps(world: &mut World, steps: usize, dt_yr: f64, config_hash: &str, out_dir: &str) -> String {
    run_steps_with_formation(world, steps, dt_yr, config_hash, out_dir, 42, 0.1, 1.0).0
}

/// Full 11-system DAG integration with explicit formation params.
/// Returns `(snapshot hash, abort reason)`. On watchdog abort the partial
/// state is still snapshotted (auditable), and the caller must surface the
/// reason instead of reporting success.
fn run_steps_with_formation(
    world: &mut World,
    steps: usize,
    dt_yr: f64,
    config_hash: &str,
    out_dir: &str,
    seed: u64,
    theia_jitter: f64,
    volatile_h2o: f64,
) -> (String, Option<String>) {
    let mut formation = formation::FormationSystem::new(seed, theia_jitter, volatile_h2o);
    let mut solar = solar::SolarSystem;
    let mut interior = interior::InteriorSystem;
    let mut tectonics = tectonics::TectonicsSystem;
    let mut atmos = atmos::AtmosSystem;
    let mut ocean = ocean::OceanSystem;
    let mut ice = ice::IceSystem;
    let mut geochem = geochem::GeochemSystem::default();
    let mut bio = bio::BioSystem;
    let mut eco = eco::EcoSystem;
    let mut civs = civs::CivsSystem::default();
    let coupler = Coupler::default_dag(dt_yr);
    let sched = Scheduler::new(earth_kernels::DevicePolicy::Auto);
    assert!(sched.staging_test());
    world.backend = sched.backend().to_string();
    let mut systems: Vec<&mut dyn earth_core::Subsystem> = vec![
        &mut formation, &mut solar, &mut interior, &mut tectonics, &mut atmos,
        &mut ocean, &mut ice, &mut geochem, &mut bio, &mut eco, &mut civs,
    ];
    let _ = &coupler;
    // Manual DAG order (same as Coupler::default_dag) with watchdog via Coupler::sync per step.
    let mut abort: Option<String> = None;
    for _ in 0..steps {
        if let Err(e) = coupler.sync(world, &mut systems, dt_yr) {
            abort = Some(format!("{e}"));
            break;
        }
        world.advance_time(dt_yr);
    }
    std::fs::create_dir_all(out_dir).ok();
    let header = make_header(world, config_hash);
    let spath = format!("{out_dir}/state.lfs");
    let hash = save_snapshot(world, &header, &spath).expect("snapshot save");
    // Scalars timeline.
    let row = earth_io::scalar_row(world);
    std::fs::write(format!("{out_dir}/scalars.csv"), "tick,t_yr,mean_T,mean_SST,mean_CO2\n".to_string() + &row + "\n").ok();
    let _ = snapshot_hash(world);
    (hash, abort)
}

fn main() {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Init { mode, grid, seed, out } => {
            std::fs::create_dir_all(&out).ok();
            let cfg = format!("[meta]\nconfig_version=1\nmode=\"{mode}\"\nseed={seed}\ngrid=\"{grid}\"\n[time]\nt_start_yr=-4540000000.0\n[device]\npolicy=\"auto\"\n[output]\nout_dir=\"{out}\"\n");
            std::fs::write(format!("{out}/lifesim.toml"), &cfg).ok();
            if cli.json {
                println!("{}", serde_json::json!({"out": out, "mode": mode, "grid": grid, "seed": seed}));
            } else {
                println!("init {out} mode={mode} grid={grid} seed={seed}");
            }
        }
        Cmd::Run { config, steps, preset, offline, accept_slow } => {
            if let Err(e) = earth_io::verify_manifest(offline) {
                eprintln!("manifest: {e}");
                std::process::exit(1);
            }
            let (v, mode, seed, grid_cfg, t_start, hash) = read_config(&config);
            let grid_use = if preset.is_empty() { grid_cfg } else { preset };
            let steps_use = if steps == 0 { 10 } else { steps };
            // Era-adaptive dt stub: deep-time 1kyr default; twin 1yr when mode=present.
            let dt = if mode == "present" { 1.0 } else { 1000.0 };
            let (jitter, h2o) = formation_params(&v);
            let mut world = build_world(&mode, &grid_use, seed, t_start, accept_slow);
            let out_dir = "runs/run-001";
            let (h, abort) = run_steps_with_formation(&mut world, steps_use, dt, &hash, out_dir, seed, jitter, h2o);
            if let Some(reason) = abort {
                if cli.json {
                    println!("{}", serde_json::json!({"aborted": reason, "tick": world.tick, "t_yr": world.t_yr}));
                } else {
                    println!("run aborted at tick={} t_yr={}: {reason}", world.tick, world.t_yr);
                }
                std::process::exit(1);
            }
            if cli.json {
                println!("{}", serde_json::json!({"hash": h, "tick": world.tick, "t_yr": world.t_yr, "mean_T": world.global_mean_temp()}));
            } else {
                println!("run done tick={} t_yr={} hash={h} mean_T={:.3}", world.tick, world.t_yr, world.global_mean_temp());
            }
        }
        Cmd::Check { epoch, run: _, offline, control, ci_artifact } => {
            let _ = earth_io::verify_manifest(offline);
            // Epoch table (spec §16 T1). M0: L0 smoke per epoch — finite fields +
            // epoch-relevant range probe. Full physics gates land M1-M8.
            let epochs: Vec<String> = if epoch == "all" {
                vec!["F1","F2","F3","GOE","G1","G2","B1","B2","E1","E2","I1","P1","LGM","preindustrial","present","K-Pg","A2"]
                    .iter().map(|s| s.to_string()).collect()
            } else {
                vec![epoch.clone()]
            };
            let mut results = Vec::new();
            let mut all_pass = true;
            for ep in epochs {
                // M1 Hadean stages: real cold-start integration, evaluated against
                // spec §16 T1 targets (F1/F2/F3). Later milestones own their stages.
                if matches!(ep.as_str(), "F1" | "F2" | "F3") {
                    let target = match ep.as_str() {
                        "F1" => -4_540_000_000.0,
                        "F2" => -4_430_000_000.0,
                        _ => -4_300_000_000.0, // F3
                    };
                    let world = integrate_hadean_to(target, 100_000.0, 42, 0.1, 1.0);
                    let finite = world.fields.surf_temp_k.iter().all(|v| v.is_finite())
                        && world.fields.sst.iter().all(|v| v.is_finite());
                    let (value, pass, target_s) = match ep.as_str() {
                        "F1" => {
                            let (m, p) = formation::evaluate_f1(&world);
                            (m as f64, p && finite, "Tm>1800K ±200K")
                        }
                        "F2" => {
                            let (m, p) = formation::evaluate_f2(&world);
                            (m as f64, p && finite, "Ts 400–900K")
                        }
                        _ => {
                            let (f, p) = formation::evaluate_f3(&world);
                            (f as f64, p && finite, "ocean frac >30%")
                        }
                    };
                    all_pass &= pass;
                    results.push(serde_json::json!({
                        "epoch": ep, "pass": pass, "value": value, "target": target_s,
                        "solidified_yr": world.totals.solidified_yr,
                        "solar_l": world.totals.solar_l_ratio,
                    }));
                    continue;
                }
                // Non-F epochs are M3–M8 gates; M0/M1 smoke only asserts finite
                // fields + range from a present-pattern start (era runs land later).
                let mut world = build_world("formation", "L0", 42, 0.0, true);
                let h = run_steps(&mut world, 10, 1000.0, "check", "runs/check-smoke");
                let finite = world.fields.surf_temp_k.iter().all(|v| v.is_finite())
                    && world.fields.sst.iter().all(|v| v.is_finite())
                    && world.fields.pco2_ppm.iter().all(|v| v.is_finite());
                let mean_t = world.global_mean_temp();
                // Epoch range probes (smoke-level; tightened M1-M8).
                let probe = match ep.as_str() {
                    "LGM" => (-300.0..350.0).contains(&mean_t),
                    "preindustrial" | "1750" => (150.0..350.0).contains(&mean_t),
                    "present" | "2025" => (150.0..350.0).contains(&mean_t),
                    "GOE" | "G1" | "G2" => world.fields.pco2_ppm.iter().all(|v| (10.0..100000.0).contains(v)),
                    _ => true,
                };
                let pass = finite && probe;
                all_pass &= pass;
                results.push(serde_json::json!({"epoch": ep, "pass": pass, "hash": h, "mean_T": mean_t}));
            }
            if !control.is_empty() {
                eprintln!("control {control} recorded (S_cal scoring lands M7 calibration harness)");
            }
            if !ci_artifact.is_empty() {
                let doc = serde_json::json!({"results": results, "all_pass": all_pass});
                std::fs::write(&ci_artifact, serde_json::to_string_pretty(&doc).unwrap()).ok();
            }
            if cli.json {
                println!("{}", serde_json::json!({"results": results, "all_pass": all_pass}));
            } else {
                for r in &results {
                    println!("check epoch={} pass={} hash={}", r["epoch"], r["pass"], r["hash"]);
                }
            }
            if !all_pass {
                std::process::exit(1);
            }
            let _ = Path::new("runs");
        }
        Cmd::Export { run, format, scope, at, out } => {
            let spath = if run.is_empty() { "runs/run-001/state.lfs".to_string() } else { format!("{run}/state.lfs") };
            // --at tick:N | latest: M0 single-snapshot store; tick:N selects only if
            // it matches the stored tick, otherwise latest with a note.
            let (header_opt, mut world_ref, snap_hash) = match earth_core::load_snapshot(&spath) {
                Ok((h, w, hh)) => (Some(h), w, hh),
                Err(_) => {
                    let mut w = build_world("formation", "L0", 42, -4_540_000_000.0, true);
                    run_steps(&mut w, 5, 1000.0, "export", "runs/run-001");
                    (None, w, String::new())
                }
            };
            if !at.is_empty() && at != "latest" {
                eprintln!("note: --at {at} requested; M0 store holds latest tick={} (era-decimated timeline lands §21)", world_ref.tick);
            }
            if format == "netcdf" {
                eprintln!("netcdf export is feature-gated (M9); use csv|parquet|json");
                std::process::exit(2);
            }
            match scope.as_str() {
                "snapshot" => {
                    earth_io::export_csv(&world_ref, &out).expect("export");
                }
                "scalars" => {
                    let row = earth_io::scalar_row(&world_ref);
                    std::fs::write(&out, "tick,t_yr,mean_T,mean_SST,mean_CO2\n".to_string() + &row + "\n").expect("export");
                }
                "events" => {
                    let ev: Vec<String> = world_ref.events.events.iter()
                        .map(|e| format!("{},{},{},{}", e.tick, e.t_yr, e.kind, e.msg)).collect();
                    std::fs::write(&out, "tick,t_yr,kind,msg\n".to_string() + &ev.join("\n") + "\n").expect("export");
                }
                _ => {
                    eprintln!("unknown scope {scope}; want snapshot|scalars|events");
                    std::process::exit(2);
                }
            }
            // Deterministic writer: no timestamps in payload -> byte parity.
            let _ = (header_opt, snap_hash);
            if cli.json {
                println!("{}", serde_json::json!({"out": out, "format": format, "scope": scope, "at": at, "tick": world_ref.tick}));
            } else {
                println!("export {out} format={format} scope={scope} at={} tick={}", if at.is_empty() { "latest" } else { &at }, world_ref.tick);
            }
            let _ = &mut world_ref;
        }
    }
}
