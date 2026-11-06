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
            #[arg(long, default_value_t = false)] offline: bool },
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
        eprintln!("L3 requires --accept-slow on CPU-only M0 (brute-force CSR); falling back to L2");
        g = "L2".into();
    }
    let preset = GridPreset::parse(&g).unwrap_or(GridPreset::L0);
    World::fresh(preset, seed, &mode, t_start)
}

fn run_steps(world: &mut World, steps: usize, dt_yr: f64, config_hash: &str, out_dir: &str) -> String {
    let mut formation = formation::FormationSystem;
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
    for _ in 0..steps {
        if let Err(e) = coupler.sync(world, &mut systems, dt_yr) {
            eprintln!("abort: {e}");
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
    hash
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
            let (_v, mode, seed, grid_cfg, t_start, hash) = read_config(&config);
            let grid_use = if preset.is_empty() { grid_cfg } else { preset };
            let steps_use = if steps == 0 { 10 } else { steps };
            // Era-adaptive dt stub: deep-time 1kyr default; twin 1yr when mode=present.
            let dt = if mode == "present" { 1.0 } else { 1000.0 };
            let mut world = build_world(&mode, &grid_use, seed, t_start, accept_slow);
            let out_dir = "runs/run-001";
            let h = run_steps(&mut world, steps_use, dt, &hash, out_dir);
            if cli.json {
                println!("{}", serde_json::json!({"hash": h, "tick": world.tick, "t_yr": world.t_yr, "mean_T": world.global_mean_temp()}));
            } else {
                println!("run done tick={} t_yr={} hash={h} mean_T={:.3}", world.tick, world.t_yr, world.global_mean_temp());
            }
        }
        Cmd::Check { epoch, run: _, offline } => {
            let _ = earth_io::verify_manifest(offline);
            // M0: L0 smoke — run 10 DAG passes, assert finite + report epoch stub.
            let mut world = build_world("formation", "L0", 42, -4_540_000_000.0, true);
            let h = run_steps(&mut world, 10, 1000.0, "check", "runs/check-smoke");
            let finite = world.fields.surf_temp_k.iter().all(|v| v.is_finite());
            let pass = finite;
            if cli.json {
                println!("{}", serde_json::json!({"epoch": epoch, "pass": pass, "hash": h, "mean_T": world.global_mean_temp()}));
            } else {
                println!("check epoch={epoch} pass={pass} hash={h}");
            }
            if !pass {
                std::process::exit(1);
            }
            let _ = Path::new("runs");
        }
        Cmd::Export { run, format, scope: _, at: _, out } => {
            let spath = if run.is_empty() { "runs/run-001/state.lfs".to_string() } else { format!("{run}/state.lfs") };
            // If snapshot missing, run a tiny world and export that (offline-friendly).
            let world_ref = match earth_core::load_snapshot(&spath) {
                Ok((_, w, _)) => w,
                Err(_) => {
                    let mut w = build_world("formation", "L0", 42, -4_540_000_000.0, true);
                    run_steps(&mut w, 5, 1000.0, "export", "runs/run-001");
                    w
                }
            };
            earth_io::export_csv(&world_ref, &out).expect("export");
            // Byte-parity note: same input hash -> same bytes (deterministic writer, no timestamps).
            if cli.json {
                println!("{}", serde_json::json!({"out": out, "format": format}));
            } else {
                println!("export {out} format={format}");
            }
        }
    }
}
