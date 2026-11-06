# LifeSim / Earth Simulator — Specification v1.1
**Status:** DRAFT — iterative improvement in progress (background reviewers: science/arch/compute/validation/gpui)
**Started:** 2026-11-06 07:27 UTC (min 1h → satisfied 08:45 UTC; continuing to science integration) | **Repo:** https://github.com/Velmet44/lifesim (public, commit regularly)
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
    earth-core/      # FACADE re-exporting leaves: earth-units/grid/time/rng/traits/couple/snap (one-way DAG; subsystems dep earth-traits only; orchestrator earth-sim owns World::step)
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
- Presets: L0=642, L1=2562, **L2=10242 default**, L3=40962 (`Real` preset, high-end only, GPU required unless `--accept-slow`). `Low`=L0, `High`=L2.
- Dual representation: cells (SoA fields) + edges (u/v currents, C-grid-ish separate `EdgeSoA`) + vertices (render mesh rebuilt from cells, never simulated).
- **Memory (corrected per compute review):** ~30 f32/cell ≈128B/cell → L2 fields ~1.31MB + CSR ~0.29MB + verts ~0.16MB ≈ **1.7MB resident** (L3 ≈7MB). **Snapshots:** `full()` ~1.5MB, `lite()` ~10-field ~400KB, compressed archival (zstd) ~300–500KB L2. Track all three in `perf/`. Memory is non-constraint; bottleneck is bandwidth + dispatch.
- Fields per cell (f32 unless noted): elevation_m, crust_thickness_km, crust_age_Myr, plate_id(u16), mantle_heat_flux, surf_temp_K, atm: T, p, q, ocean: SST, SSS, MLD, ice_frac/thick, pCO2_ppm, pO2, pCH4, alkalinity, DIC, nutrients_P, biomass pools (microbial, veg, fauna, marine), species_richness(u32), land_use_frac, civ_presence/tech/energy/emissions. f64 accumulators (carbon/energy/water totals, era time) live in separate `Totals`, not SoA. No f16.
- SoA: struct-of-`Vec<f32>` 64B-aligned, one Vec per FieldId (stable u16 IDs in canonical registry — arch reviewer table TODO M0), `area_m2` precomputed (sums to 4πR² asserted at build). Neighbors: precomputed CSR (`csr_off[N+1]`, `csr_nbr[~6N]`), fixed order for determinism. WGSL: one StorageBuffer per field.
- Neighbor iterators precomputed (CSR). Area-weighted reductions use fixed-order tiled pattern (TILE=1024, indexed slots, serial final fold — never `par_iter().sum()`); see §20.

### 5.2 Time Manager
- Adaptive per-subsystem dt + Coupler phase DAG (not single sync tick). Era regimes map to phase tables (Hadean/Myr, Phanerozoic/kyr, Twin/daily subcycled). Erosion subcycled 1:10.
- Fast systems (atmos/ocean) subcycle between slow handshakes. Watchdog: CFL/diag checks + conservation audit after every phase; halve dt + retry once then abort with event-log entry (never silent clamp). Checks → auto-dt shrink + log.
- Deterministic seeded RNG (splitmix64 / xoshiro, versioned streams per subsystem). Two-tier determinism: bitwise-identical only within (version,grid,seed,backend,algo) via fixed-order tiled reductions (never par_iter().sum()); cross-backend CPU vs GPU = statistical equality. RNG per-subsystem blake3->Xoshiro streams.
- Timeline log: every event {t, type, cell/region, magnitude, cause-chain}.

### 5.3 Kernel Abstraction (Hybrid CPU/GPU)
```rust
trait Kernel { fn name(&self)->&str; fn kind(&self)->KernelKind; fn reads(&self)->&[FieldId]; fn writes(&self)->&[FieldId]; fn run_cpu(&self,g:&GridAccess,p:&KernelParams)->SimResult<Diagnostics>; fn wgsl_module(&self)->Option<&str>; } // None=CPU-only; precompiled module name, fixed neighbor order, no atomics
```
- Runtime DevicePolicy: Auto | CpuOnly | GpuPreferred | Hybrid. Probe adapters; software/no-GPU/staging-test-fail -> Cpu. Hybrid places fused stencil groups on GPU only if resident_steps x gain > 2x migration cost; any gpu err -> log + CPU rerun (mark backend=cpu-fallback).
- CPU: rayon pool sized N-2 (reserve UI + wgpu poll) + autovectorized SIMD (no nightly std::simd); deterministic indexed tiles + serial final fold.
- GPU: wgpu compute for FUSED stencil groups with device-resident subcycling (atmos dyn+rad, ocean advect-diffuse, mantle diffuse, SIA-lite, erosion stencil w/o drainage sort); one WGSL module per family + shared grid.wgsl CSR; workgroup 128-256, 1 cell/thread, SoA StorageBuffers; staging budget <=10% step (40KB L2 field; naive per-kernel shuttle banned). CPU stays: plate rotation/boundaries, drainage sort, phylogeny/agents/events, reductions audit, IO.
- Perf harness perf/ (criterion kernels/coupler/e2e benches L0-L3 x CPU/GPU + budgets.toml; CI perf-check +-15% warn/+-30% fail). See S12 for dt-qualified budgets.

### 5.4 Coupling
- Field registry (FieldId u16 stable IDs + units + ranges + single writer per phase; Coupler::new panics on overlapping writes). Phase DAG: [tectonics dt]->[climate subcycle]->[weathering+geochem]->[bio/eco]->[audit]; audit after every phase
- Conservation audit each phase (energy/water/carbon f64 Totals); imbalance > tol => Err(Conservation) -> watchdog retry/abort, never silent clamp.

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

## 8. Validation & Calibration (research core — v1.2 integrated from validation review)
- **Hindcasts (required):** H-a 1850→2025 (full industrial, prescribed GHG/aerosol/solar/volc), H-b 1900→2020 rolling 20-yr windows every 10yr, H-c 2005→2025 Argo-era holdout (no tuning on H-c). Anomaly baselines stated per test (1850–1900 or 1991–2020).
- **Ensemble tiers:** IC n≥20 (perturbed IC) · PARAM LHS n≥32 (weathering_k, outgas, albedo, ECS-proxy, AMOC sens) · STOCH seed sweep n≥10. Deep-time: LHS n≥64 lite-grid L0/L1 + n≥8 L2 confirmation of control only; GP emulator + Sobol published. Control = PARAM best + IC mean; report spreads separately.
- **Scoring:** `z=(m-o)/tol` (log: dex), `S=sqrt(mean(w·z²))+0.1·D_Maha(theta,prior)`; pass iff `S_cal≤1.0` + no range-gate fail + H-c coverage ok. Publish per-checkpoint z table in CI. See §16 T1 weights; cal/val split — val failures documented, no retuning without version bump.
- **Twin gates:** M7 blocked on T3 20-yr sanity (bias<0.2K yr1–5, trend bias<0.05K/dec, RMSE<0.15K, no >0.5K drift, energy <0.5W/m², carbon closure <2%, rank-hist flat, coverage 80–95%); M8 blocked on T2 1900–2025 repro (CO2 ±4ppm, ΔT ±0.15K, OHC ±30%, ice trend ±5pp/dec, SLR ±5cm, pop ±3%/energy ±10% scenario-labeled).
- **Forcings:** prescribed Meinshausen GHG + Toohey AOD + SATIRE solar + MACv2 aerosols for H-a/H-b; free civ-emissions runs tagged counterfactual.
- **Data manifest:** `data/manifest.toml` {url,doi,sha256,version,license,fetched_utc}; CI `--offline` runs on pinned embedded fallbacks, `--online` verifies checksums; never vendor restricted bytes.
- **Ethics (civs):** SCENARIO-not-forecast labeling, bands-first; no sub-global targeting (global+4 lat bands max); collapse scenarios opt-in + banner; deep-future (>10kyr) SPECULATIVE watermark; no infra coupling.
- Every subsystem has `docs/provenance/*.md` with equations + citations.

## 9. GPUI App (`earth-gpui`) — All View Modes (v1.2 integrated from gpui review)
- **Embedding decision (Risk #1):** spike both day-1, **M0 ships Path B (render-to-texture fallback)**; promote to Path A (native `gpui::Surface`/Blade interop Element) only if spike passes with no GPUI fork. `cargo tree -i wgpu` must show single version (pinned to GPUI re-export). Render thread owns its Adapter/Device/Queue.
- **Threads:** sim ↔ render ↔ UI, no shared mut. Sim→UI bounded `sync_channel(8)` keep-latest (dropped-previews counter in status bar); UI→Sim unbounded; transfer `Arc<SnapshotView>` (published fields only). UI cmds apply at sync-tick boundary + logged (replayable). See §21 for enums.
- **Views (`views/`):** GlobeView (3D icosphere mesh, per-frame uniform+texture only) · MapView (2D equirect + ortho, CPU-raster for headless tests) · TimelineView (era-segmented log scale, decimated — scalars first <100ms, textures on demand) · DashboardView (scalar ring-buffers) · InspectorView (cell fields + ≤4096-pt sparkline) · ScenarioEditor (next-tick applies) · EnsembleView (mean±band + member selector) · ExportDialog (1:1 CLI parity, byte estimate).
- **Credibility color rules:** no rainbow; fixed vmin/vmax per layer+era with clipped-legend ticks; shared `earth-core::colormap` LUT for GUI+CLI; defaults: hypsometric elev, RdBu diverging T-anomaly (±8K twin / ±20K deep-time), white-blue ice, viridis/plasma biomass, inferno civ-energy; colorblind-safe default on.
- **Keyboard-first:** Space pause/resume, [/] or ←/→ ∓1 step, Shift ∓10, 1/2 globe/map, 3..7 right tabs, L layer, E export, ? help, q quit (confirm), s snapshot-now; single FocusHandle, modal focus trap, all actions buttoned.
- UX: UI event latency <100ms p95 at full sim speed; watchdog dot + dropped-preview counter surfaced.

## 10. CLI (`earth-cli`) — headless-first, canonical; GUI shells it
- Versioned `lifesim.toml` (`config_version`, hashed into outputs). All cmds `--json`-capable.
- `init --mode formation|present|forward --grid L2 --seed --out`, `run --config [--steps] [--preset Low]`, `ensemble -n --vary k:min..max`, `check --epoch K-Pg|GOE|LGM|preindustrial|present --run`, `export --format parquet|csv|netcdf --scope snapshot|scalars|events --at`, `calibrate --budget --target --out control-run.toml`.
- **M0 spike gates:** Path-B globe ≥2fps L0 CPU-only + golden screenshot; single wgpu version; determinism byte-equal rerun; timeline scalars <100ms load / <200ms seek; export parity byte-compare; keyboard playback headless without mouse.

## 11. Determinism, Repro, Versioning
- Triple versioning: engine_version (Cargo) != snapshot_version (u32 layout) != config_version (TOML [sim] version); every output embeds all three + blake3(canonical TOML) + backend/determinism/adapter/threads. Snapshots: single .lfs file (magic LFS1 + postcard header + body + blake3 + zstd); phylo tree as separate append-only .tree.lfs. CI: CpuOnly rerun hash-equal; GPU within-tol; checkpoint tests per milestone.

## 12. Performance Budgets (to be measured, tracked in perf/)
- Targets at L2, mid quad-core CPU-only (to be replaced by perf/ actuals): deep-time >=1 Myr/wall-s at dt=1kyr with erosion 1:10 (PASS ~2.5 Myr/s typical); climate >=10 sim-yr/wall-s at DAILY dt with EMBM/dynamics-lite (6-hourly only High/GPU preset; 6-hourly FAILS ~4x CPU-only); twin year <5 wall-s daily. GPU 2-5x only fused resident high-ALU kernels; Laplacians ~1.2-2x L2. Staging <=10% step. Measure on reference quad-core before freezing.

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

## 16. Checkpoint Table (see §8 canonical; legacy v1.1 table kept for traceability — validation reviewer T1 supersedes tolerances/weights)
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

## 20. Determinism & Snapshot Protocol v1.2 (main-agent pass)
- RNG: per-subsystem `RngStream(tag, seed, step_counter)` via splitmix64; streams versioned (`rng_v1`). No wall-clock / thread-id entropy.
- Reductions: chunk→fixed-order tree merge (CPU) / workgroup-reduce→ordered host merge (GPU). No atomic-float accumulation.
- Snapshot header (JSON): `{sim_version, grid, seed, config_hash, mode, t_yr, rng_state, kernel_versions}` + body bincode+zstd. Hash (blake3) of decoded fields stored for `earth-cli check --determinism` (rerun → same hash).
- CI: `determinism-smoke` (L0, 100 sync ticks, 2 reruns, hash equal) blocks merge.

## 21. CLI & App Protocol v1.2 (sketch)
```bash
earth-cli init --mode present --grid L2 --seed 7 --config base.toml
earth-cli run --steps 1000 --checkpoint-every 100 --out data/runs/run-001/
earth-cli ensemble --n 64 --param ensemble/lhs.toml --score checkpoint-rmse
earth-cli check --epoch LGM --run data/runs/run-001/
earth-cli export --format parquet --layers surf_temp,ice_frac,biomass --out out/
```
- GUI↔sim messages (mpsc, never blocks UI):
```rust
enum SimCmd { Pause, Resume, StepOnce, SetSpeed(f64), ScrubTo(f64/*yr*/), SetCO2(f64), InjectImpact{lat:f32,lon:f32,d_km:f32}, Reseed(u64) }
enum SimEvt { Tick{yr:f64}, Checkpoint(String), Warning(String), Done{hash:String} }
```
- Timeline decimation: store every Nth tick full + every tick globals; scrubber interpolates globals, loads nearest full on pause.

## 22. UQ & Ethics Note v1.2
- Twin ensembles report mean±P10–P90 bands, never single-line forecasts; document forcing assumptions (SSP-like scenarios table TODO).
- Civilization outputs labeled *scenario projections, not predictions of human behavior*; no personally identifiable data; no autonomous policy triggers. Deep-future civ collapse paths presented with uncertainty + sensitivity attribution.

## Appendix A: Open Questions for Next Iterations
- Spectral vs finite-volume atmos core final choice after M3 perf.
- Phylogenetic tree storage format (custom vs newick+sqlite).
- Ensemble UQ method (EnKF vs particle vs simple Gaussian bands for twin).
- Exact embedded fallback dataset minimal set.
