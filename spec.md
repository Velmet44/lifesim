# LifeSim / Earth Simulator — Specification v1.1
**Status:** DRAFT — iterative improvement in progress (background reviewers: science/arch/compute/validation/gpui)
**Started:** 2026-11-06 07:27 UTC (target: ≥2h refinement loop, end ≥09:27 UTC) | **Repo:** https://github.com/Velmet44/lifesim (public, commit regularly)
**Stack:** Rust + GPUI + wgpu | **Default grid:** 10,242 cells (icosphere) | **Purpose:** research-cum-prediction

## 1. Vision & Purpose
Build a scientific earth simulator that simulates Earth from formation (~4.54 Gya) including every physically simulable subsystem on an average computer, used for:
1. **Retrospective science:** cold-start from formation, reproduce real Earth history through calibration.
2. **Digital-twin near-future prediction:** warm-start from real present-day state, predict decades–centuries with ensembles + uncertainty bands.
3. **Deep-future projection:** run calibrated Earth forward Myr–Gyr (next supercontinent, solar brightening, future extinctions).

Not a game. No god-mode interventions as core loop (scenario editor allowed for experiments). Credibility > visual flash.

## 2. Explicit Non-Goals
- No molecular / atom-by-atom simulation.
- No weather-forecast-grade NWP parity, no genome-level biology, no real satellite-imagery fidelity.
- No fantasy physics; only published, citable schemes with provenance docs.
- No pixel-perfect present Earth; success = statistically Earth-like + calibrated control run close to real timeline.

## 3. Three Init Modes (all required)
- **(a) Cold start `formation`:** solar nebula → accretion → magma ocean → Hadean. Seed determines stochastic events.
- **(b) Digital twin `present`:** initialize from real datasets (ERA5-like T/S, observed CO2, topography/bathymetry, biomass, land-use) → predict 2026–2100+.
- **(c) Deep future `forward`:** take calibrated present or end-of-cold-run state → run forward with solar evolution.
- All modes share same engine, field registry, event log. Mode selects initializers + era regimes + default dt policy.

## 4. Workspace Layout
```
lifesim/
  spec.md
  Cargo.toml (workspace)
  crates/
    earth-core/      # time, grid, fields, traits, coupling, events, rng, checkpoint
    earth-kernels/   # Kernel trait + CPU(rayon+SIMD) / GPU(wgpu compute) backends, auto-select + fallback
    formation/ solar/ interior/ tectonics/ atmos/ ocean/ ice/ geochem/ bio/ eco/ civs/
    earth-io/        # datasets fetch/cache/embed/import, snapshots, timeline log, export parquet/csv/netcdf
    earth-cli/       # headless runs, ensembles, sweeps, calibration harness
    earth-gpui/      # desktop app
  data/ (gitignored cache) + assets/embedded-fallbacks/
  perf/ (benches + budgets)
  docs/provenance/ (per-subsystem literature + equations)
```

## 5. Core Design

### 5.1 Grid
- Icosphere geodesic (Loop subdivision of icosahedron), cell-centered finite-volume.
- Presets: L0=642, L1=2562, **L2=10242 default**, L3=40962 (`Real` preset, high-end only), plus `Low` alias.
- Dual representation: cells (SoA fields) + vertices for rendering mesh.
- Fields per cell (f32 unless noted): elevation_m, crust_thickness_km, crust_age_Myr, plate_id(u16), mantle_heat_flux, surf_temp_K, atm: T, p, q, u/v (edge), ocean: SST, SSS, MLD, currents, ice_frac/thick, pCO2_ppm, pO2, pCH4, alkalinity, DIC, nutrients_P, biomass pools (microbial, veg, fauna, marine), species_richness(u32), land_use_frac, civ_presence/tech/energy/emissions.
- Memory budget: ~400KB per full snapshot at 10k cells (f32 SoA + u16/u32 aux). Snapshots compressed (zstd) + decimated timeline.
- Neighbor iterators precomputed (CSR). Area-weighted reductions in fixed order for determinism.

### 5.2 Time Manager
- Adaptive per-subsystem dt + global sync ticks. Era-aware regimes: Hadean (Myr steps ok), Phanerozoic (kyr), Twin-mode (yr/day subcycled).
- Fast systems (atmos/ocean) subcycle between slow handshakes. Watchdog: CFL/diag checks → auto-dt shrink + log.
- Deterministic seeded RNG (splitmix64 / xoshiro, versioned streams per subsystem). Same seed+version+grid = bit-identical (fixed-order reductions, no atomics nondeterminism).
- Timeline log: every event {t, type, cell/region, magnitude, cause-chain}.

### 5.3 Kernel Abstraction (Hybrid CPU/GPU)
```rust
trait Kernel { fn name()->&str; fn cpu(&self, ctx)->Result<()>; fn gpu_wgsl()->Option<&str>; fn cost_model()->Cost; }
```
- Runtime `DevicePolicy`: `Auto | CpuOnly | GpuPreferred | Hybrid`. Probe wgpu adapters; fallback to CPU if no GPU.
- CPU: rayon parallel + SIMD (wide / std::simd when stable, else scalar autovectorized loops).
- GPU: wgpu compute shaders for stencil kernels (atmos dynamics, ocean advect, mantle diffusion, erosion). CPU+GPU mixed: scheduler assigns heaviest kernels to GPU, rest CPU, with explicit staging budget.
- Perf harness `perf/` tracks sim-years/wall-sec per tier; budgets published per milestone.

### 5.4 Coupling
- Field registry with units + valid ranges. Exchange ticks aligned: e.g., every 1 kyr (deep-time) tectonics→elevation→climate→weathering→geochem→bio.
- Conservation checks (energy, water, carbon) each sync; imbalance > tol → warn + clamp + log.

## 6. Subsystems

### 6.1 Solar / Astro (`solar`)
- Solar luminosity L(t)=L0/(1+0.4*(1-t/t0)) approx (Gough 1981) + user override.
- Orbit: eccentricity/obliquity/precession (Berger/Laskar-lite or prescribed Milankovitch cycles in twin/Quaternary mode).
- Lunar recession → day length + tides (affects mixing param).

### 6.2 Formation (`formation`)
- 4.56–4.45 Gya: planetesimal accretion energy + giant impacts (Theia as scripted energy injection w/ seed jitter) → magma ocean (1000–2000K) → solidification + outgassing (H2O/CO2) → first crust.
- Output: initial T field, volatile inventory, magma-ocean solidification time checkpoint (~4.43 Gya).

### 6.3 Interior / Mantle (`interior`)
- 0-D/1-D parameterized convection: mantle temp Tm, heat flux Q = k*(Tm-Ts)^n, radiogenic decay (U/Th/K).
- Tracks core dynamo on/off → magnetosphere flag (atmos escape modifier).

### 6.4 Tectonics (`tectonics`)
- Semi-rigid plates: N plates (8–14 major + micro), Euler poles evolving via mantle-torques + slab-pull param.
- Per-step: move plates (rigid rotation on sphere), detect convergent/divergent/transform boundaries, create: subduction (recycling + arc volcanism CO2), collision orogeny (uplift), rifting (new crust + subsidence), mid-ocean ridges.
- Isostasy + erosion (stream-power-lite: dh/dt = uplift - K*A^m*S^n - hillslope diffusion).
- Validation: continental configs vs Scotese paleomaps (qualitative), hypsometry, crustal growth curve.

### 6.5 Atmosphere (`atmos`)
- Coarse hydrostatic primitive-equation / energy-moisture-balance hybrid: start EMBM + dynamical-core-lite (barotropic vorticity + Hadley-param), upgrade path to T21 spectral.
- Radiation: band model for CO2/H2O/CH4/O2/O3 + solar zenith + albedo (ice/cloud/land).
- Moist convection (Betts-Miller-lite), large-scale precip, clouds diagnostic (RH-based) + forcing.
- Subcycled dt hours–days; time-averaged to coupling tick.

### 6.6 Ocean (`ocean`) + Ice (`ice`)
- 2-layer / shallow-water + thermohaline box-param (AMOC strength ∝ density gradient), wind-driven gyres from atmos stress.
- SST/SSS advection-diffusion, MLD diagnostic, sea-ice thermodynamics (Semtner 0-layer), land ice sheets (SIA-lite for glacial cycles).
- Milankovitch + CO2 must produce ~100kyr/41kyr glacial cyclicity in Quaternary test.

### 6.7 Geochemistry (`geochem`)
- Carbon cycle: atmos-ocean-biosphere box + silicate weathering feedback F_w = k * runoff * exp((T-T0)/Te) * veg_factor; carbonate burial, volcanic outgassing from tectonics/LIPs.
- Oxygen: burial - weathering + H escape; must allow GOE ~2.4 Ga when cyanobacteria + sinks align.
- Nutrients P as limiting; track pH/alkalinity crudely for ocean acidification in twin mode.

### 6.8 Life / Evolution (`bio`)
- Abiogenesis gate: requires liquid water + T 273–373K + hydrothermal/pond habitat + reducing→neutral chemistry; hazard rate per suitable cell per Myr; seeded. Expected window 4.0–3.5 Ga in control run.
- Genome: trait vector (e.g., 16–64 dims: T_opt, pH_opt, photosyn flag, O2 tolerance, size, etc.) + neutral barcode. Mutation + selection (fitness = environment match * competition).
- Speciation: allopatric (isolation) + adaptive radiation when empty niches open post-extinction. Phylogenetic tree persisted (parent links, divergence t, extinction t).
- Major transitions as threshold unlocks: anoxygenic photo → oxygenic (Great Oxidation linkage), eukaryogenesis (rare, O2-gated), multicellularity, land colonization (O3-shield-gated).

### 6.9 Ecosystems (`eco`)
- Per-cell biomass pools by functional group (producers/consumers/decomposers marine/terrestrial), NPP = f(PAR,T,water,nutrients,CO2 fertilize), consumption Lotka-Volterra-lite + carrying capacity.
- Dispersal between neighbors. Collapse cascades when NPP crash or T excursion > tolerance → extinction pulse recorded to tree.

### 6.10 Civilizations (`civs`, dormant until thresholds)
- Activation requires: high O2, temperate climate, biomass surplus, tech-prereq species traits (tool-use proxy). In cold runs may never trigger or trigger for non-humans — allowed, logged.
- Agent layer per civ-cell: pop (logistic + tech), tech level (energy use, 0–3 scale), land-use, fossil emissions (CO2/CH4), aerosols, nuclear/anthropogenic forcings.
- Twin-mode calibration: must reproduce 1900–2025 CO2/temp/pop/energy within bands given observed forcings.
- Deep-future: tech growth capped by physics + climate feedback (collapse possible).

### 6.11 Events
- Types: impacts (size-freq power law + scripted Chicxulub-analog option), LIPs/supervolcanoes (VEI8), superplumes, Snowball bifurcations, anoxic events, biotic innovations, gamma/cosmic (rare).
- Seeded + reproducible; scripted triggers available for validation (e.g., force 66Ma impactor in test).

## 7. Data Pipeline (`earth-io`)
- Sources: topography/bathymetry (ETOPO/GEBCO-lite resampled), ERA5 climatology for twin init, observed GHG curves, Scotese paleomaps for validation overlays, paleo-CO2/temp syntheses (Foster/Veizer-style compilations), species/biomass baselines.
- Layers: `fetch` (online w/ cache + checksum) → `embedded` (minimal fallback constants in-repo) → `import` (user NetCDF/CSV/GeoTIFF via pluggable readers).
- Export: Parquet/CSV primary, NetCDF converter, snapshot format v1 (bincode+zstd + json header with version/seed/grid/mode).
- Licensing: track provenance + license per dataset; no redistribution of restricted data; scripts fetch at build/run.

## 8. Validation & Calibration (research core)
- **Checkpoint harness** (`earth-cli check --epoch <name>`): asserts global means within tolerance:
  4.54Ga accretion energy, 4.43Ga solidification, 4.3Ga oceans, 3.5Ga stromatolites/life present, 2.4Ga GOE (O2 rise), 720–635Ma Snowball episodes reachable, 538Ma Cambrian radiation (richness spike), 252Ma Permian crash, 66Ma K-Pg impact→biomass collapse, 20ka LGM ice extent, preindustrial 280ppm, present ~420ppm + ~1.2K anomaly.
- **Auto-calibration:** ensemble sweeps (Latin hypercube) over key params (weathering k, outgassing, albedo, mutation rates) → score vs checkpoints → publish `control-run.toml` nearest Earth.
- Twin-mode 20-yr hindcast must stay sane; industrial-era repro gate blocks Civs milestone.
- Every subsystem has `docs/provenance/*.md` with equations + citations.

## 9. GPUI App (`earth-gpui`) — All View Modes
- 3D globe via embedded wgpu surface (M0 spike; fallback render-to-texture). Layers: elevation/bathymetry, T anomaly, ice, aridity, biomass, species richness, civ energy/night-lights proxy.
- 2D equirectangular + orthographic maps. Timeline scrubber with epoch ticks + event markers. Dashboard charts (CO2,O2,T,ice,biomass,richness,pop). Cell inspector (all fields + history sparkline). Playback (pause/scrub/speed), scenario editor (CO2 slider, inject impact/eruption, reseed), ensemble view (mean±band), export dialog.
- UX: keyboard-first, async sim thread (mpsc), UI never blocks; progress + watchdog warnings surfaced.

## 10. CLI (`earth-cli`)
- `init --mode formation|present|forward`, `run --steps --seed --grid --preset`, `ensemble --n --param`, `check --epoch`, `export --format`, `calibrate --budget`.
- Headless-first; GUI optional. Config TOML versioned.

## 11. Determinism, Repro, Versioning
- Sim version + grid + seed + config hash in every output. Fixed-order reductions. CI: rerun determinism test (same hash), checkpoint tests per milestone.

## 12. Performance Budgets (to be measured, tracked in perf/)
- Target at 10k cells, mid quad-core CPU-only: deep-time ≥1 Myr / wall-sec (tectonics+geochem+eco-lite), climate subcycle ≥10 sim-yr / wall-sec when active, twin-mode full year <5 wall-sec (tunable fidelity). GPU should give 2–5× on stencil kernels. Document actuals; do not promise.

## 13. Milestones & Exit Gates
- M0 scaffold+globe spike: globe renders flat field; reruns bit-identical.
- M1 solar+formation: Hadean cooling plausible; orbit cycles correct.
- M2 tectonics v1: continents vs paleomaps qualitative; hypsometry sane.
- M3 atmos-ocean-ice: glacial cycles emerge; 2×CO2 sensitivity 2–4.5K.
- M4 geochem+events: GOE emerges in control; Snowball reachable.
- M5 bio: abiogenesis 4.0–3.5Ga window; Cambrian-like radiation.
- M6 eco/extinctions: K-Pg analog → matching collapse.
- M7 data+calibration+twin init: warm start matches present; 20-yr ensemble sane.
- M8 civs+deep-future: 1900–2025 repro; 1-Gyr run stable.
- M9 polish: L3 grid, nested regional climate, NetCDF, plugin API.

## 14. Risks
1. GPUI×wgpu embedding — spike early, fallback ready.
2. Numerical drift over Gyr — era regimes + watchdog + auto-dt.
3. Calibration non-identifiability — priors + sensitivity analysis.
4. Scope creep — feature flags + Low/High/Real presets.
5. Data licensing — fetch-at-run, provenance tracked.

## 15. Improvement Log
- v1.0 2026-11-06 07:27 UTC: initial comprehensive draft. Refinement subagents launched.
- v1.1 2026-11-06 07:33 UTC: main-agent pass — checkpoint tolerances table, kernel/DevicePolicy table, config schemas, trait sketches, repo live at Velmet44/lifesim. Background reviews running.
- (append entries below as refinements land)

## 16. Checkpoint Table v1.1 (quantitative, to be tightened by validation reviewer)
| Epoch | Observable | Control-run target | Tolerance (initial) | Gate blocks |
|---|---|---|---|---|
| 4.54 Ga | accretion energy / magma ocean onset | Tm>1800K global | ±200K | M1 |
| 4.43 Ga | solidification | crust present, Ts<800K | Ts 400–900K | M1 |
| 4.30 Ga | oceans | liquid water fraction >30% cells 273–373K | ±15pp | M1 |
| 3.50 Ga | life present | abiogenesis done, microbial biomass>0 | binary | M5 |
| 2.40 Ga | GOE | pO2 rise 1e-5→1e-3 PAL order | log ±1 | M4 |
| 720–635 Ma | Snowball reachable | ice_frac>80% in perturbed ensemble | binary reachability | M4 |
| 538 Ma | Cambrian radiation | richness 5–20× pre-edge | factor 2 | M5 |
| 252 Ma | P-T crash | marine biomass -70–-95% under LIP forcing | ±15pp | M6 |
| 66 Ma | K-Pg | biomass -50–-80% under 10km impactor | ±15pp | M6 |
| 20 ka | LGM ice | land-ice extent 1.5–2.5× preind (N.Hemi) | ±30% | M3 |
| 1750 | preindustrial | 280ppm CO2, ΔT=0 baseline | ±10ppm | M7 |
| 2025 | present | ~420ppm, ΔT ~+1.1–1.3K vs 1850–1900 | CO2 ±5ppm, T ±0.2K | M7/M8 |
| 2×CO2 | ECS diagnostic | 2.0–4.5K | range gate | M3 |

Scoring: weighted RMSE in normalized units; control-run = min score over LHS ensemble (n≥64 deep-time-lite, n≥20 twin). Publish `control-run.toml` + seed + hash.

## 17. Kernel & DevicePolicy v1.1
| Kernel | Grid op | CPU strat | GPU candidate | Precision |
|---|---|---|---|---|
| atmos_dyn (advect/vort) | stencil 1-ring | rayon chunks | YES wgpu primary | f32 |
| atmos_rad (band) | pointwise | rayon+SIMD | YES (embarrassingly parallel) | f32 |
| ocean_advect/diffuse | stencil | rayon | YES | f32 |
| ice_thermo | pointwise | rayon | no (branchy, stay CPU) | f32 |
| mantle_diffuse | stencil/1-D | rayon | YES | f32 |
| plate_rotate | rigid mat | serial CPU | no | f64 accum, f32 store |
| erosion/stream-power | stencil+flow | rayon | MAYBE (needs prefix/flow routing → CPU first) | f32 |
| weathering/geochem box | reductions | serial/rayon reduce fixed-order | no | f64 accum |
| bio/eco per-cell | pointwise+neighbor disperse | rayon | MAYBE phase 2 | f32 + u32 ids |
| civ agents | sparse agents | serial CPU | no | f32/f64 mix |

```rust
#[derive(Clone,Copy)] pub enum DevicePolicy { Auto, CpuOnly, GpuPreferred, Hybrid }
pub trait Kernel {
  fn name(&self)->&'static str;
  fn run_cpu(&self, g:&GridViewMut, p:&Params)->Result<Diagnostics>;
  fn wgsl(&self)->Option<&'static str>; // None = CPU-only
  fn supports_gpu(&self)->bool { self.wgsl().is_some() }
}
pub struct Scheduler { policy: DevicePolicy, staging_budget_mb: usize }
// Auto: probe adapter; if none → CpuOnly. Hybrid: GPU for stencil list above, CPU rest, overlap via double-buffer.
// Determinism: GPU reductions use fixed-order tree (workgroup reduce → ordered merge on CPU); versioned in snapshot header.
```

Memory re-check (10,242 cells): 40 f32 fields ≈ 40*10k*4B ≈ 1.6MB resident; snapshot (all + aux u16/u32 + tree refs) ~1–2MB raw, ~400KB zstd — original claim holds as *compressed* size. Document both.

## 18. Config Schemas v1.1 (TOML sketches)
```toml
[sim] mode="formation"|"present"|"forward" grid="L2" seed=42 version=1
[time] sync_kyr=1.0 watchdog=true max_dt_kyr=100.0
[devices] policy="Auto" staging_mb=256
[formation] theia_jitter=0.1 volatile_H2O=1.0 # Earth oceans
[tectonics] plates=12 euler_drift=0.05 orogeny_m_per_kyr=50
[atmos] core="embm+dycore-lite" rad_bands=8 clouds="rh-diag"
[ocean] layers=2 amoc_sens=1.0
[geochem] weathering_k=1.0 outgas_scale=1.0
[bio] trait_dim=32 mutation=1e-3 abiogenesis_hazard=1e-6
[eco] dispersal=0.05
[civs] enabled="auto" tech_cap=3.0
[data] fetch="auto" cache="data/cache" embedded_fallback=true
[ensemble] n=64 method="lhs" score="checkpoint-rmse"
```

## 19. Core Traits v1.1 (sketch for arch reviewer to finalize)
```rust
pub trait Subsystem {
  fn id(&self)->&'static str; fn dt_range(&self)->(f64,f64); // sim-yr
  fn step(&mut self, g:&mut Grid, t:SimTime, dt:f64, rng:&mut RngStream)->Result<Diagnostics>;
  fn handshake(&self)->Vec<FieldId>; // fields it reads/writes
}
pub struct Coupler { order: Vec<&'static str>, sync_interval_yr: f64 }
impl Coupler { pub fn sync(&self, g:&mut Grid)->Result<ConservationReport>; }
```

## Appendix A: Open Questions for Next Iterations
- Spectral vs finite-volume atmos core final choice after M3 perf.
- Phylogenetic tree storage format (custom vs newick+sqlite).
- Ensemble UQ method (EnKF vs particle vs simple Gaussian bands for twin).
- Exact embedded fallback dataset minimal set.
