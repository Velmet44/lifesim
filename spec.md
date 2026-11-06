# LifeSim / Earth Simulator — Specification v1.3
**Status:** DRAFT — iterative improvement in progress (background reviewers: science/arch/compute/validation/gpui)
**Started:** 2026-11-06 07:27 UTC | **Phase 2:** 08:46 UTC → 11:46 UTC (3h refinement, commits ~15min, NO sleep/wait loops) | **Repo:** https://github.com/Velmet44/lifesim
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
- Canonical presets (grid × physics; `--preset` overrides `--grid`; `--grid` alone keeps Standard physics):
| Preset | Grid | Atmos dt | Subcycling | GPU | Use |
|---|---|---|---|---|---|
| Low | L0 642 | EMBM annual | none | never | smoke tests |
| Standard (default) | L2 10242 | daily dynamics-lite | full phase DAG | preferred | dev + control runs |
| High | L2 10242 | 6-hourly | full phase DAG | preferred (required for 6-hourly headroom) | M3 ECS attribution |
| Real | L3 40962 | 6-hourly | full phase DAG | required (refuse without adapter unless `--accept-slow`) | final/high-end only |
- Dual representation: cells (SoA fields) + edges (u/v currents, C-grid-ish separate `EdgeSoA`) + vertices (render mesh rebuilt from cells, never simulated).
- **Memory (corrected per compute review):** ~30 f32/cell ≈128B/cell → L2 fields ~1.31MB + CSR ~0.29MB + verts ~0.16MB ≈ **1.7MB resident** (L3 ≈7MB). **Snapshots:** `full()` ~1.5MB, `lite()` ~10-field ~400KB, compressed archival (zstd) ~300–500KB L2. Track all three in `perf/`. Memory is non-constraint; bottleneck is bandwidth + dispatch.
- Fields per cell (f32 unless noted): elevation_m, crust_thickness_km, crust_age_Myr, plate_id(u16), mantle_heat_flux, surf_temp_K, atm: T, p, q, ocean: SST, SSS, MLD, ice_frac/thick, pCO2_ppm, pO2, pCH4, alkalinity, DIC, nutrients_P, biomass pools (microbial, veg, fauna, marine), species_richness(u32), land_use_frac, civ_presence/tech/energy/emissions. f64 accumulators (carbon/energy/water totals, era time) live in separate `Totals`, not SoA. No f16.
- SoA: struct-of-`Vec<f32>` 64B-aligned, one Vec per FieldId (stable u16 IDs in canonical registry — arch reviewer table TODO M0), `area_m2` precomputed (sums to 4πR² asserted at build). Neighbors: precomputed CSR (`csr_off[N+1]`, `csr_nbr[~6N]`), fixed order for determinism. WGSL: one StorageBuffer per field.
- Neighbor iterators precomputed (CSR). Area-weighted reductions use fixed-order tiled pattern (TILE=1024, indexed slots, serial final fold — never `par_iter().sum()`); see §20.

### 5.2 Time Manager
- Adaptive per-subsystem dt + Coupler phase DAG (not single sync tick). Era regimes map to phase tables (Hadean/Myr, Phanerozoic/kyr, Twin/daily subcycled). Erosion subcycled 1:10.
- Fast systems (atmos/ocean) subcycle between slow handshakes. Watchdog: CFL/diag checks + conservation audit after every phase; halve dt + retry once then abort with event-log entry (never silent clamp). Checks → auto-dt shrink + log.
- Deterministic seeded RNG (blake3(seed||tag||tick)→Xoshiro256PlusPlus, one stream per subsystem; splitmix64 retired). Two-tier determinism: bitwise-identical only within (version,grid,seed,backend,algo) via fixed-order tiled reductions (never par_iter().sum()); cross-backend CPU vs GPU = statistical equality. RNG per-subsystem blake3->Xoshiro streams.
- Timeline log: every event {t, type, cell/region, magnitude, cause-chain}.

### 5.3 Kernel Abstraction (Hybrid CPU/GPU)
```rust
trait Kernel { fn name(&self)->&str; fn kind(&self)->KernelKind; fn reads(&self)->&[FieldId]; fn writes(&self)->&[FieldId]; fn run_cpu(&self,g:&GridAccess,p:&KernelParams)->SimResult<Diagnostics>; fn wgsl_module(&self)->Option<&str>; } // None=CPU-only; precompiled module name, fixed neighbor order, no atomics
```
- Runtime DevicePolicy: Auto | CpuOnly | GpuPreferred | Hybrid. Probe adapters; software/no-GPU/staging-test-fail -> Cpu. Hybrid places fused stencil groups on GPU only if resident_steps x gain > 2x migration cost; any gpu err -> log + CPU rerun (mark backend=cpu-fallback).
- CPU: rayon pool sized N-2 (reserve UI + wgpu poll) + autovectorized SIMD (no nightly std::simd); deterministic indexed tiles + serial final fold.
- GPU: wgpu compute for FUSED stencil groups with device-resident subcycling (atmos dyn+rad, ocean advect-diffuse, mantle diffuse, SIA-lite, erosion stencil w/o drainage sort); one WGSL module per family + shared grid.wgsl CSR; workgroup 128-256, 1 cell/thread, SoA StorageBuffers; staging budget <=10% step (40KB L2 field; naive per-kernel shuttle banned). CPU stays: plate rotation/boundaries, drainage sort, phylogeny/agents/events, reductions audit, IO.
- Perf harness perf/ (criterion kernels/coupler/e2e benches L0-L3 x CPU/GPU + budgets.toml; CI perf-check +-15% warn/+-30% fail). See §12 for dt-qualified budgets.

### 5.4 Coupling
- Field registry (FieldId u16 stable IDs + units + ranges + single writer per phase; Coupler::new panics on overlapping writes). Phase DAG: [tectonics dt]->[climate subcycle]->[weathering+geochem]->[bio/eco]->[audit]; audit after every phase
- Conservation audit each phase (energy/water/carbon f64 Totals); rel-tols per phase — energy 1e-5, water 1e-6, carbon 1e-6; geochem additionally |sources−sinks|<1% throughput (§23#2). Imbalance > tol => Err(Conservation) -> watchdog retry/abort, never silent clamp.

## 6. Subsystems

### 6.1 Solar / Astro (`solar`)
- Luminosity (Gough 1981, τ = Gyr since ZAMS, τ⊙ = 4.57): `L(τ) = L⊙ / (1 + 0.40·(1 − τ/τ⊙))`, L⊙ = 3.828e26 W (TSI₀ = 1361 W/m²). Lookback: `L(t_lb) = L⊙ / (1 + 0.40·t_lb/τ⊙)`. Valid τ ∈ [0,8] Gyr; beyond τ⊙+1 Gyr blend to Bahcall+2001 track via override table (no linear extrapolation past +1.5 Gyr). Checks: L(4.38Ga)=0.728±0.01 L⊙; L(2.5Ga)=0.821±0.015; dL/dt|₀=+0.087±0.01 L⊙/Gyr.
- Spectrum: EUV/XUV `F_EUV(t) = F_EUV,₀·(t/4.57Gyr)^(−1.2±0.2)`, t≥0.1 Gyr (Ribas+2005; Tu+2015), saturates 100× modern below 0.1 Gyr → feeds H-escape (geochem) + magnetopause (interior).
- Orbit: Berger (1978) / Laskar+2004 lite — ecc 95/125/400 kyr, obliquity 41 kyr (22.1–24.5°), precession 19/23 kyr. Quaternary phase error < ±2 kyr (41 kyr band), amplitude ±10%.
- Lunar: tidal `da/dt` anchored to LLR 3.83 cm/yr; day length from angular-momentum conservation (Hadean day 5–8 h; 2.45 Ga 16–18 h). Tidal mixing κ_tide ∝ a^(−3), normalized to Munk-Wunsch 1–2 TW.

### 6.2 Formation (`formation`)
- 4.56–4.45 Gya: planetesimal accretion energy + giant impacts (Theia as scripted energy injection w/ seed jitter) → magma ocean (1000–2000K) → solidification + outgassing (H2O/CO2) → first crust.
- Output: initial T field, volatile inventory, magma-ocean solidification time checkpoint (~4.43 Gya).

### 6.3 Interior / Mantle (`interior`)
- Parameterized convection (Schubert-Turcotte-Olson; Davies): `Q = Q₀·(Tm/Tm₀)^(1+β)·(η(Tm₀)/η(Tm))^β`, β=0.30±0.05, η(Tm)=η₀·exp(A·(1/Tm−1/Tm₀)), A≈3e4 K. Anchor Q₀=47±2 TW (Davies & Davies 2010), Tm₀=1620±50 K potential T (Herzberg). Radiogenics H(t)=Σ H₀,i·exp(−λᵢ·t_lb): ²³⁸U/²³⁵U/²³²Th/⁴⁰K (McDonough & Sun 1995 BSE); Hadean Q ≈ 3–5× present.
- Dynamo (Stevenson 2003; Loper): on iff Q_cmb > Q_ad (5–15 TW) AND Rm=μ₀σvL > 40; report dipole M (present 7.8e22 A·m²) + magnetopause R_mp (≈10 R_e modern), not binary flag. Escape: Jeans λ_esc + EUV hydrodynamic branch (Erkaev/Owen), XUV-saturated pre-3.5 Ga.

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
- AMOC (Stommel 1961 two-box + Rooth): `ψ = k_ψ·(β·ΔS − α·ΔT) + ψ_wind`, k_ψ → 16–18 Sv preindustrial (Lumpkin & Speer); hysteresis required (bistable under FWF 0.05–0.3 Sv). Wind gyres via Sverdrup `β·V = curl(τ_wind)/ρ₀`.
- SST/SSS advection-diffusion, MLD diagnostic, Semtner 0-layer sea ice (enthalpy flux-form across subcycle handshake for conservation audit).
- Ice sheets: Glen n=3 SIA + Weertman sliding + isostatic τ≈3–5 kyr (Peltier VM5a) + Schoof (2007) grounding-line flux (MISI). LGM target: −120±20 m SLE, NH ice 1.5–2.5× preindustrial volume (ICE-6G).
- Quaternary test (Ganopolski/Abe-Ouchi class): 41 kyr pre-MPT (2.6–1.2 Ma) → ~100 kyr post-MPT (spectral peak 80–120 kyr + sawtooth) with regolith-removal + dust (α_dust +0.02–0.05). Orbit+CO₂-only runs EXPECTED to fail MPT (negative control); orbital termination phase tol ±5 kyr, amplitude ±30 m SLE.

### 6.7 Geochemistry (`geochem`)
- Silicate weathering (continental, Walker-Urey-WHK, area-integrated): `F_sil = k_sil·f_land·(R/R₀)^α·(pCO₂/pCO₂,₀)^β·exp((T−T₀)/Te)·f_veg`, α=0.5±0.2, β=0.25±0.1, Te=13.7±2 K, f_veg=1 abiotic → 2–4 vascular (Devonian+). k_sil → preindustrial F_sil=6–9 Tmol C/yr at 280 ppm/288 K.
- Seafloor weathering (required, Sleep & Zahnle 2001 / KT15): `F_sea = k_sea·(Q/Q₀)^m·(pCO₂/p₀)^γ·exp(−E_sea/RT_pore)`, m=0.5–1.0, γ=0.2–0.4, modern F_sea=1–4 Tmol C/yr. Dominates when f_land<0.1.
- Outgassing: `F_out = F_out,₀·(Q/Q₀)^n_out·(1+LIP(t)/F_out,₀)`, F_out,₀=6–10 Tmol C/yr, n_out=0.5–1.0. Carbonate burial = sink at steady state; CCD/pH from rebuilt alkalinity+DIC (Zeebe & Wolf-Gladrow K1/K2(T,S)).
- Oxygen (redox-closed, Catling & Kasting Ch.10–13): `dO₂/dt = f_org·F_C,burial − (F_red + F_met + F_esc)`, f_org≈0.15–0.25; F_red scales to F_out with mantle fO₂ evolution; H-escape from EUV + cold-trap. GOE gate: MIF-S loss (Δ³³S<0.1‰) 2.45–2.32 Ga; pO₂ <1e-6 PAL → 1e-6–1e-2 overshoot (±1 dex stage boundaries, timing ±0.05 Gyr) → Proterozoic hold <1e-1 PAL until NOE ≥0.8 Ga.
- Validation: preindustrial 280±10 ppm (calibration; EMBM error floor — ±5 aspirational only); Archean steady state pCO₂ 1e3–1e5 ppm at T 270–300 K; Phanerozoic GEOCARB track ±0.5 dex.
- Nutrients P (+Fe marine) Liebig-limiting; pH/alkalinity for twin acidification.

### 6.8 Life / Evolution (`bio`)
- Abiogenesis (staged gate, all required per cell): (i) liquid water a_w>0.6, 280≤T≤340 K sustained ≥10 kyr; (ii) habitat: alkaline-vent ΔpH≥2 (Russell) OR subaerial wet–dry cycling (Damer-Deamer); (iii) redox: pH₂>1e-4 bar or Fe²⁺>1e-6 mol/kg (HCN/formamide proxy; Sutherland/Patel); (iv) UV/impact frustration: no D>100 km sterilizing reset within 1 Myr, surface UV<1e3× modern if subaerial.
- Hazard: `λ = λ₀·f_hab` per suitable-cell/Myr, λ₀=1e-4–1e-2 log-uniform LHS, calibrated so P(first success ∈ [4.0,3.5] Ga)=50–95% (publish N_suit(t) curve + impact-reset log; reject λ₀ with P>99% before 4.3 Ga or P<5% by 3.5 Ga). Bounds: 4.1 Ga Bell / 3.7 Ga Nutman / 3.5 Ga Schopf (contested flags).
- Genome: trait vector (16–64 dims) + neutral barcode. Fitness (Sharpe-Schoolfield TPC): `w = w_max·g(T;T_opt,CT_min,CT_max)·h(pH,O₂,UV)·(1−Σ α_ij N_j/K_i)`; mandatory trade-offs (O₂-tolerance ↔ anaerobe growth Δr≥10%; phototrophy ↔ nutrient half-sat +50%).
- Speciation: allopatric + post-extinction radiation; persistent tree (parent, birth, death).
- Transitions (O₂ in PAL): oxygenic photo (Mn-cluster proxy, local oases pre-GOE); eukaryogenesis rare (λ_euk≤1e-3× speciation, O₂≥1e-3 PAL 10 Myr); land (column O₃≥50% modern ⇔ O₂≥1e-2 PAL 10 Myr mean; Berkner-Marshall/Ratner-Walker/Kasting).

### 6.9 Ecosystems (`eco`)
- NPP (per PFT, Liebig minimum): `NPP = ε_max·min(L(PAR), f_T, f_W, f_N, f_CO₂)·(1−r_auto)`, L=α·PAR/(PAR+K_PAR) (K_PAR≈30–80 W/m²), f_T=TPC as §6.8, f_CO₂=1+β·ln(C/C₀) β=0.3–0.6 (FACE/Norby), ε_max per PFT (C3/C4/microbial/marine).
- Consumption: Holling-II `C = a·N·P/(1+a·h·N)` (never bare Lotka-Volterra). Dispersal between neighbors.
- Collapse iff NPP<20% baseline ≥1 kyr OR T outside CT limits ≥90% of range cells → extinction pulse to tree.

### 6.10 Civilizations (`civs`, dormant until thresholds)
- Activation requires: high O2, temperate climate, biomass surplus, tech-prereq species traits (tool-use proxy). In cold runs may never trigger or trigger for non-humans — allowed, logged.
- Agent layer per civ-cell: pop (logistic + tech), tech level (energy use, 0–3 scale), land-use, fossil emissions (CO2/CH4), aerosols, nuclear/anthropogenic forcings.
- Twin-mode calibration: must reproduce 1900–2025 CO2/temp/pop/energy within bands given observed forcings.
- Deep-future: tech growth capped by physics + climate feedback (collapse possible).

### 6.11 Events
- Types: impacts (size-freq power law + scripted Chicxulub-analog option), LIPs/supervolcanoes (VEI8), superplumes, anoxic events, biotic innovations, gamma/cosmic (rare). Snowball is NEVER injected — emergent only (probe below).
- Snowball bifurcation probe (Voigt & Abbot 2012; Pierrehumbert; Hoffman & Schrag): runaway declared iff ice-line <25° sustained ≥2 kyr with d(ice-line)/dt<0 at fixed forcing. Sturtian probe: 717 Ma palaeogeography + 0.94 L⊙ + pCO₂ 200→40 ppm; entry required in 130–40 ppm; exit via 0.01–0.1 bar CO₂ over 3–30 Myr. Metric: ice-line latitude + global T + thickness class (thin<2m vs thick>10m). M4 gate (deeptime control): entry as pCO₂ is drawn down 200→40 ppm in ≥3/10 perturbed members AND deeptime control has zero spontaneous Snowballs outside 720–635 Ma AND §23#4 1-D Budyko-Sellers hysteresis cross-check shows two stable branches.
- Seeded + reproducible; scripted triggers available for validation (e.g., force 66Ma impactor in test). K-Pg chain (Toon): 10 km / 1–3e24 J → AOD>10 for >3 yr → PAR −90%, land T −5–10 K → NPP collapse; biomass −50–−80% must FOLLOW from eco equations, not imposed kill fraction; separate Deccan pre-stress run required.

## 7. Data Pipeline (`earth-io`)
- Sources: topography/bathymetry (ETOPO/GEBCO-lite resampled), ERA5 climatology for twin init, observed GHG curves, Scotese paleomaps for validation overlays, paleo-CO2/temp syntheses (Foster/Veizer-style compilations), species/biomass baselines.
- Layers: `fetch` (online w/ cache + checksum) → `embedded` (minimal fallback constants in-repo) → `import` (user NetCDF/CSV/GeoTIFF via pluggable readers).
- Export: Parquet/CSV primary, NetCDF converter (feature-gated), snapshot single-file `.lfs` v1 (magic LFS1 + postcard header + body + blake3 + zstd; see §11/§20; bincode+sidecar-JSON retired).
- Licensing: track provenance + license per dataset; no redistribution of restricted data; scripts fetch at build/run.

## 8. Validation & Calibration (research core — v1.2 integrated from validation review)
- **Hindcasts (required):** H-a 1850→2025, H-b 1900→2020 rolling 20-yr windows every 10yr, H-c 2005→2025 holdout (no tuning on H-c). **Baseline convention:** ΔT ≡ 0 at 1850–1900 for ALL twin gates (T2/T3); 1750 row reports absolute CO₂ only (no ΔT baseline); per-test baseline printed in every check output. No 1991–2020 ΔT baselines in gates.
- **Ensemble tiers:** IC n≥20 · PARAM LHS n≥32 · STOCH n≥10. Deep-time: LHS n≥64 lite-grid L0/L1 + n≥8 L2 confirmation. **Controls (split):** `control-deeptime.toml` = single best trajectory+seed (min S_cal; all M1–M6/G1/G2 gates evaluated on it); `control-twin.toml` = ensemble mean+spread around PARAM-best (all T2/T3/H-c gates on it). Never one file for both estimators.
- **Scoring:** `z=(m-o)/tol` (log vars: dex), `S=sqrt(sum w·z²/sum w)+λ·D_Maha(theta,prior)`, λ=0.1 with Ledoit-Wolf shrinkage reported; pass iff `S_cal≤1.0` + no range-gate fail + H-c coverage ok; `S_val` reported separately (report-only, no gate). Weights w = T1 column in §16. Publish per-checkpoint z table in CI; cal/val split — val failures documented, no retuning without version bump.
- **Twin gates:** M7 blocked on T3 20-yr sanity (bias<0.2K yr1–5, trend bias<0.05K/dec, RMSE<0.15K, no >0.5K drift, energy <0.5W/m², carbon closure <2%, rank-hist flat, coverage 80–95%); M8 requires full T2 (OHC ±30%, Sep ice ±5pp/dec, SLR total 20±5cm, pop ±3%/energy ±10% scenario-labeled) + 1-Gyr soak. M7 blocks M8.
- **T2 protocol (1900–2025 repro):** prescribed forcings only (G1/A1/S1/V1 tables); metrics: CO₂ annual ±10ppm, ΔT ±0.2K (1850–1900), OHC ±30%, Sep ice ±5pp/dec, SLR 20±5cm total, pop ±3%/energy ±10% (scenario-labeled); obs must lie in 5–95% band; formal table M0.
- **T3 protocol (20-yr sanity, init 2005 → 2025, IC n≥20):** |bias yr1–5|<0.2K, trend bias<0.05K/dec, RMSE<0.15K drift-corrected, no member drift>0.5K, energy<0.5W/m² decadal, carbon closure<2%, rank-hist χ² p>0.05, coverage 80–95% nominal 90%. Formal table M0.
- **Forcings:** prescribed Meinshausen GHG + Toohey AOD + SATIRE solar + MACv2 aerosols for H-a/H-b; free civ-emissions runs tagged counterfactual.
- **Data manifest:** `data/manifest.toml` {url,doi,sha256,version,license,fetched_utc}; CI `--offline` runs on pinned embedded fallbacks, `--online` verifies checksums; never vendor restricted bytes.
- **Ethics (civs):** SCENARIO-not-forecast labeling, bands-first; no sub-global targeting of civ *outcomes* in exports/scoring (global+4 lat bands; no per-region ranking; cell-level GUI inspection allowed with SCENARIO watermark); collapse scenarios opt-in + banner; deep-future (>10kyr) SPECULATIVE watermark; no infra coupling.
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
- Triple versioning: engine_version (Cargo) != snapshot_version (u32 layout) != config_version (TOML [sim] version); every output embeds all three + blake3(canonical TOML) + backend/determinism/adapter/threads. Snapshots: single state file `state.lfs` (magic LFS1 + postcard header + body + blake3 + zstd); run dir may also hold optional append-only `phylo.tree.lfs` sidecar (see §19). Snapshot layout version = snapshot_version u32. CI: hash-equal reruns within identical (version,grid,seed,backend,adapter,algo); statistical equality across backends/adapters (|mean diff|<1e-5); checkpoint tests per milestone.

## 12. Performance Budgets (to be measured, tracked in perf/)
- Targets at L2, mid quad-core CPU-only (to be replaced by perf/ actuals): deep-time >=1 Myr/wall-s at dt=1kyr with erosion 1:10 (PASS ~2.5 Myr/s typical); climate >=10 sim-yr/wall-s at DAILY dt with EMBM/dynamics-lite (6-hourly only High/GPU preset; 6-hourly FAILS ~4x CPU-only); twin year <5 wall-s daily. GPU 2-5x only fused resident high-ALU kernels; Laplacians ~1.2-2x L2. Staging <=10% step. Measure on reference quad-core before freezing.

## 13. Milestones & Exit Gates
- M0 scaffold+globe spike: Path-B globe ≥2fps L0 CPU-only + golden screenshot; CpuOnly rerun hash-equal; single wgpu version. CI: determinism-smoke.
- M1 solar+formation: Hadean Ts 400–900K post-solidification, oceans >30% cells by 4.3Ga; orbit periods ±1% vs Berger/Laskar. CI: check --epoch F1/F2/F3.
- M2 tectonics v1: hypsometry bimodal ±15%, monotonic crustal growth; paleomap overlay qualitative review. CI: hypsometry artifact.
- M3 atmos-ocean-ice: Quaternary 41kyr pre-MPT → ~100kyr post-MPT (negative control orbit+CO₂-only must FAIL); ECS Gregory+slab 2.0–5.0K gate (very-likely, AR6), central 2.5–4.0K documented + kernel decomposition (cloud ±0.5 W/m²/K; CRE bias vs CERES ±5 W/m² or ECS claim rejected). CI: check --epoch I1/P1.
- M4 geochem+events: GOE MIF-S 2.45–2.32 Ga ±0.05 Gyr in control; Snowball entry 130–40 ppm in ≥3/10 perturbed + zero spontaneous outside 720–635 Ma. CI: check G1/G2 (cal/val split).
- M5 bio: abiogenesis 4.0–3.5Ga in control; Cambrian 5–20× richness spike. CI: check B1/B2.
- M6 eco/extinctions: LIP → marine -70–-95%; 10km impactor → -50–-80% + recovery e-fold <5Myr. CI: check E1/E2.
- M7 data+calibration+twin init: warm start within T2 present-day tols; T3 20-yr sanity green (bias/trend/RMSE/conservation/coverage). Blocks M8. CI: H-c holdout.
- M8 civs+deep-future: T2 1900–2025 repro (CO2 ±10ppm, ΔT ±0.2K); 1-Gyr stability no-crash. CI: check A2 + long soak.
- M9 polish: L3 Real preset, nested regional climate, NetCDF (feature), plugin API, perf trend published.

## 14. Risks (owner + mitigation + tripwire)
1. GPUI×wgpu embedding (owner: gpui) — spike both paths day-1, M0 ships Path B; single-wgpu-version CI gate. Tripwire: second wgpu version in `cargo tree` fails CI.
2. Numerical drift over Gyr (owner: core) — era regimes + per-phase audit + halve-retry-abort watchdog; never silent clamp. Tripwire: conservation CI red.
3. Calibration non-identifiability (owner: validation) — literature priors + Sobol + cal/val split + frozen control-run; report degeneracy (weathering_k/outgas r>0.8 → reparametrize). Tripwire: S_cal ≤1 with |r|>0.8 forces reparam issue.
4. Scope creep / EVERYTHING (owner: all) — subsystems are feature-flagged traits; Low/High/Real presets bound default cost; new fields require FieldId + writer + budget note. Tripwire: L2 resident >2.5MB or deep-time <1Myr/s blocks merge.
5. Data licensing (owner: io) — fetch-at-run + manifest checksums + pinned embedded fallbacks; `--offline` CI; never vendor restricted bytes. Tripwire: offline CI job red.
6. GPU-staging optimism (owner: compute) — resident-subcycling rule + ≤10% staging budget + startup staging_test; CPU-first claims. Tripwire: staging >10% disables GPU path with log.
7. Overconfident predictions (owner: validation) — bands-first UQ, coverage gates, SCENARIO labeling, ethics display rules. Tripwire: H-c coverage fail blocks M7/M8.

## 15. Improvement Log
- v1.0 2026-11-06 07:27 UTC: initial comprehensive draft. Refinement subagents launched.
- v1.1 2026-11-06 07:33 UTC: main-agent pass — checkpoint tolerances table, kernel/DevicePolicy table, config schemas, trait sketches, repo live at Velmet44/lifesim. Background reviews running.
- v1.2 2026-11-06 07:34–08:46 UTC: integrated gpui+validation+compute+arch — Path-B M0 render, thread proto, color rules, CLI schemas; H-a/b/c hindcasts, 3-tier ensembles, S scoring, T2/T3 gates, manifest+ethics; corrected memory (1.7MB resident/zstd 300-500KB), resident-GPU + staging<=10%, dt-qualified budgets; facade crates, exact traits, DAG coupling, .lfs snapshots, feature presets; §13/§14 testable gates + tripwires.
- v1.3 2026-11-06 ~08:50 UTC: integrated science review — pinned Gough τ-form + EUV/lunar equations; WHK+seafloor weathering + redox-closed O₂ with staged GOE gate; emergent Snowball bifurcation probe; Stommel AMOC + hysteresis stack + MPT negative control; Sharpe-Schoolfield fitness + Holling-II + staged abiogenesis λ₀; AR6 ECS gate + widened present-day CO₂ ±10ppm; K-Pg mechanism chain.
- (append entries below as refinements land)
- v1.4 2026-11-06 ~08:52 UTC: adversarial audit (20 findings) resolved — canonical RNG/snapshot/trait/control/scoring/table; preset table + precedence; staging cap 64MB; DAG-pass decimation; §10 CLI canonical; conservation rel-tols; ethics scoping. Promoted docs/twin-init.md + docs/deep-future.md (ERA5→L2 remap, prescription tables, spin-up gates, H-series wiring, R0–R3 matrix, greenhouse ladder, watermarking).

## 16. Checkpoint Table T1 (normative numbers + weights; method in §8; S uses W column)
| Epoch | Observable | Control-run target | Tolerance | Gate | W |
|---|---|---|---|---|
| 4.54 Ga | accretion energy / magma ocean onset | Tm>1800K global | ±200K | M1 | 0.5 |
| 4.43 Ga | solidification | crust present, Ts<800K | Ts 400–900K | M1 | 0.5 |
| 4.30 Ga | oceans | liquid water fraction >30% cells 273–373K | ±15pp | M1 | 1.0 |
| 3.50 Ga | life present | abiogenesis done, microbial biomass>0 | binary | M5 | 1.0 |
| 2.45–2.32 Ga | GOE (MIF-S loss) | pO₂ <1e-6 → 1e-6–1e-2 overshoot → <1e-1 hold to NOE | timing ±0.05 Gyr, stage ±1 dex | M4 | 2.0 |
| 720–635 Ma | Snowball (LEGACY row — superseded by §6.11 probe) | ice-line<25° ≥2kyr, entry 130–40ppm, ≥3/10 members | §6.11 + M4 | 1.0 |
| 538 Ma | Cambrian radiation | richness 5–20× pre-edge | factor 2 | M5 | 1.5 |
| 252 Ma | P-T crash | marine biomass -70–-95% under LIP forcing | ±15pp | M6 | 1.5 |
| 66 Ma | K-Pg chain (AOD>10 >3yr → PAR−90%) | biomass −50–−80% from eco eqns | ±12pp, recovery e-fold <5Myr | M6 | 1.5 |
| 20 ka | LGM | −120±20 m SLE; NH ice vol 1.5–2.5× preind (ICE-6G) | SLE ±20m | M3 | 2.0 |
| 1750 | preindustrial | 280ppm CO2, ΔT=0 baseline | ±10ppm | M7 | 2.0 |
| 2025 | present (annual CO₂; ΔT vs 1850–1900) | 420ppm, +1.1–1.3K | CO₂ ±10ppm, T ±0.2K | M7/M8 | 3.0 |
| 2×CO2 | ECS Gregory 150-yr + slab eq (pattern-corrected) | AR6 likely 2.5–4.0, best 3.0 | gate 2.0–5.0 + kernel breakdown | M3 | 3.0 |

Scoring: see §8 canonical (`z`, S, D_Maha + Ledoit-Wolf, S_cal≤1.0 gate, S_val report-only). Weights = T1 column below (TBD values M0, owner validation). Control files per §8 split; publish both + seeds + hashes.

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
// Canonical Kernel trait lives in §5.3 (name/kind/reads/writes/run_cpu/wgsl_module). This section is backend-mapping only.
pub struct Scheduler { policy: DevicePolicy, staging_budget_mb: usize } // budget ≤64MB; placement inequality in §5.3
// Table above = which kernels are fused resident-GPU groups vs CPU. No competing placement rule here.
```

Memory (canonical counts from field registry: ~30 f32/cell ≈128B): L2 fields ~1.31MB + CSR ~0.29MB + verts ~0.16MB ≈ 1.7MB resident total (see §5.1). Snapshots: full() ~1.5MB, lite() ~400KB, zstd ~300–500KB. §14 tripwire denominator = CPU resident total incl. CSR+verts (GPU mirrors excluded).

## 18. Config Schemas v1.1 (TOML sketches)
```toml
[sim] mode="formation"|"present"|"forward" grid="L2" seed=42 version=1
[time] slow_handshake_kyr=1.0 # full DAG pass interval (deep-time); phases+subcycles per era in docs/era-dt.md watchdog=true max_dt_kyr=100.0
[devices] policy="Auto" staging_mb=64 # ≤64MB cap; placement inequality + ≤10% step budget in §5.3
[formation] theia_jitter=0.1 volatile_H2O=1.0 # Earth oceans
[tectonics] plates=12 euler_drift=0.05 orogeny_m_per_kyr=50
[atmos] core="embm+dycore-lite" rad_bands=8 clouds="rh-diag"
[ocean] layers=2 amoc_sens=1.0
[geochem] weathering_k=1.0 outgas_scale=1.0
[bio] trait_dim=32 mutation=1e-3 abiogenesis_lambda0_per_cell_Myr=3e-4 # LHS 1e-4..1e-2 (replaces uncalibrated 1e-6)
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
pub struct Phase { pub name: &'static str, pub members: Vec<&'static str>, pub dt_yr: f64 }
pub struct Coupler { pub phases: Vec<Phase> } // sync = one full DAG pass; slow-handshake interval in [time]
impl Coupler { pub fn sync(&self, w: &mut World)->SimResult<ConservationReport>; }
```

## 20. Determinism & Snapshot Protocol v1.2 (main-agent pass)
- RNG: per-subsystem `RngStream(tag,seed,tick) = Xoshiro256PlusPlus(blake3(seed||tag||tick||rng_v))`, rng_v=1, tick = global phase counter u64 LE. No wall-clock / thread-id entropy. (splitmix64 retired everywhere.)
- Reductions (canonical algorithm): CPU = TILE=1024 indexed slots + serial final fold in cell-index order; GPU = per-workgroup partials → ordered host merge in index order. Ban `par_iter().sum()` and atomic-float accumulation. (Unifies §5.1/§5.2/§17 descriptions.)
- Snapshots: see §11 canonical (`state.lfs` + optional `phylo.tree.lfs`; postcard+zstd+blake3; JSON-header/bincode retired). Hash = blake3(decoded fields) stored for `earth-cli check --determinism`.
- CI: `determinism-smoke` (L0, 100 DAG passes, 2 reruns, hash equal) blocks merge.

## 21. CLI & App Protocol v1.2 (sketch)
```bash
lifesim init --mode present --grid L2 --seed 7 --out runs/run-001   # §10 canonical; --preset overrides --grid
lifesim run --config lifesim.toml [--steps N] [--preset Standard]
lifesim ensemble --config lifesim.toml -n 64 --vary geochem.weathering_k:0.5..2.0
lifesim check --epoch LGM --run runs/run-001
lifesim export --run runs/run-001 --format parquet --scope snapshot --at tick:1000
```
- GUI↔sim messages (mpsc, never blocks UI):
```rust
enum SimCmd { Pause, Resume, StepOnce, SetSpeed(f64), ScrubTo(f64/*yr*/), SetCO2(f64), InjectImpact{lat:f32,lon:f32,d_km:f32}, Reseed(u64) }
enum SimEvt { Tick{yr:f64}, Checkpoint(String), Warning(String), Done{hash:String} }
```
- Timeline decimation: store full `state.lfs` every Nth DAG pass (era-dependent: Hadean 1/100, Phanerozoic 1/10, Twin every pass) + scalar globals every pass; scrubber seeks nearest committed pass + fast-forwards suppressed subsystems (marked preview-interpolated).

## 22. UQ & Ethics Note v1.2
- Twin ensembles report mean±P10–P90 bands, never single-line forecasts; document forcing assumptions (SSP-like scenarios table TODO).
- Civilization outputs labeled *scenario projections, not predictions of human behavior*; no personally identifiable data; no autonomous policy triggers. Deep-future civ collapse paths presented with uncertainty + sensitivity attribution.

## 23. Additional Validation Protocols (from science review §3)
1. Faint-young-Sun closure: Archean control holds 270–300 K at L≈0.72–0.82 L⊙ via pCO₂ 1e3–1e5 ppm + CH₄ 10–1000 ppm (no prescribed T); report (L,pCO₂,CH₄,T) quad; fail = hand-tuned global albedo outside [0.25,0.35].
2. Carbonate audit: every geochem handshake |sources−sinks| <1% throughput; pH 6.5–8.5 Phanerozoic / 6.0–8.0 Archean (Halevy & Bachan); publish K1/K2(T,S) table version.
3. MIF-S proxy: Δ³³S >0.3‰ pre-2.45 Ga → <0.1‰ post-2.32 Ga; midpoint ±0.05 Gyr.
4. Hysteresis diagram: small-ice-cap instability + Snowball bifurcation as T vs pCO₂ with two stable branches (Budyko-Sellers 1-D EBM cross-check before 3-D claim).
5. MPT spectrum: Lomb-Scargle of ice-volume analog shows 41 kyr dominance 2.0–1.2 Ma → 80–120 kyr 0.8–0 Ma (p<0.05); orbit+CO₂-only negative control must NOT pass.
6. AMOC bistability: hosing 0.1/0.2/0.3 Sv ×500 yr recovery-vs-collapse map; present ψ=16–18±3 Sv (RAPID).
7. Abiogenesis N_suit(t) curve + impact-reset log published; control P-window 50–95%.
8. Phylogeny sanity: post-K-Pg doubling ≤5 Myr; Cambrian 5–20× over ≤30 Myr; P-T marine loss 70–95% under Traps-class forcing (Burgess: CO₂+warming+anoxia).
9. Score conditioning: Ledoit-Wolf shrinkage reported for D_Maha; S_cal and S_val reported separately; split `control-deeptime.toml` (single trajectory+seed) vs `control-twin.toml` (ensemble mean+spread).

## 24. Cross-Subsystem Couplings (wire before M3–M5)
- EUV → H-escape → O₂ net; L+orbit → insolation → wind stress → AMOC/greyre mixing (κ_tide from lunar a).
- Q_cmb → plate speeds v∝(Q/Q₀)^γ γ≈0.5–1 (Conrad & Lithgow-Bertelloni) → outgassing + orogeny weathering pulses (Raymo-Ruddiman) → CO₂ → ice entry.
- Palaeogeography → ocean gateways (Drake/Tasman/Panama switches) + spreading-rate seafloor weathering; LIP(t) → SO₂/aerosol + CO₂ pulse → eco collapse.
- O₂ → O₃ column (Kasting photochemistry lookup) → UV mortality + land gate; O₂ → CH₄ lifetime (CH₄∝1/OH(O₂)) → Archean greenhouse.
- Burial f_org ↔ DIC/Alk + O₂ source; NPP ↔ weathering f_veg (2–4× at root/vascular/myco innovations).
- Ice → sea level → shelf area → burial/weathering area + albedo; dust ↔ radiation + ocean Fe fertilization.
- Impacts/LIPs → stratospheric AOD (Toohey, e-fold 1–3 yr) → T + MLD + PAR; anoxia (P-input × stratification) → selective extinction.

## 25. Glossary
(Twin init: docs/twin-init.md. Deep-future: docs/deep-future.md.)
 (abbreviations used in gates)
PAL (present atmospheric level, O₂=0.21 bar) · SLE (sea-level equivalent, m) · MPT (Mid-Pleistocene Transition ~0.9 Ma) · EMBM (energy-moisture-balance model) · SIA (shallow-ice approximation) · MISI (marine-ice-sheet instability) · ECS (equilibrium climate sensitivity, Gregory 150-yr + slab) · LHS (Latin hypercube) · EnKF (ensemble Kalman filter) · TPC (thermal performance curve, Sharpe-Schoolfield) · PFT (plant functional type) · AOD (aerosol optical depth) · f_org (organic burial fraction) · MIF-S (mass-independent S fractionation, Δ³³S) · GOE/NOE (Great/Neoproterozoic Oxidation Events) · BSE (bulk silicate Earth) · ZAMS (zero-age main sequence) · TSI (total solar irradiance) · LLR (lunar laser ranging) · CRE (cloud radiative effect) · FWF (freshwater flux, Sv) · IC/PARAM/STOCH (ensemble tiers) · H-a/b/c (hindcasts) · L0–L3 (grid presets 642/2562/10242/40962).

## 26. Satisfaction Checklist (stop conditions for the refinement loop)
- [x] All 5 background reviews landed + integrated (gpui v1.2, validation v1.2, compute v1.2, arch v1.2, science v1.3).
- [x] Consistency sweep: single story for RNG, snapshots, ECS, CO₂ tols, Snowball, kernels (v1.3.1).
- [x] Twin-init + deep-future drafts integrated as docs/ (v1.4).
- [x] Adversarial audit integrated (v1.4, 20/20 findings).
- [ ] Wall clock ≥11:46 UTC 2026-11-06 (phase-2 3h window).

## Appendix A: Open Questions (status 08:46 UTC, phase 2)
- [DECIDED v1.2] Atmos core: start EMBM + dynamics-lite finite-volume; T21 spectral only if M3 perf demands (see §5.3).
- [DECIDED v1.2] Phylo tree: flat append-only `.tree.lfs` sidecar (parent/birth/death), not newick+sqlite per-tick.
- [DECIDED v1.2] UQ: twin EnKF/particle with rank-histogram + coverage gates; deep-time LHS + GP emulator + Sobol.
- [DECIDED v1.2] Embedded fallbacks pinned in `data/manifest.toml` (means + gradients + splines, §8 + manifest).
- [DONE v1.3] Solar τ-form + EUV/lunar; WHK β/Te + seafloor; staged GOE; Snowball 130–40ppm probe; abiogenesis λ₀ LHS.
- [PENDING M0] Field-registry value sign-off; conservation rel-tols; WGSL entry-point inventory.
- [PENDING M0] Field-registry value ranges final sign-off; conservation rel-tols per quantity; WGSL entry-point + workgroup inventory.
