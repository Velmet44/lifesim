# Deep-Future Forward-Mode Protocol (DRAFT — SPECULATIVE)

> **Status:** Scratch draft. **All deep-future (>50 Myr forward) outputs are SPECULATIVE.**
> Every artifact produced under this protocol MUST carry the `SPECULATIVE` watermark
> (see §6). No code here — protocol only. Intended consumer: a future
> implementation task + a reviewer checking the 1-Gyr stability run.

Scope: forward integration of the lifesim world from present-day initial
conditions out to **+1 Gyr**, with solar brightening, a next-supercontinent
prescription, and greenhouse-limit watchdogs. Anything beyond ~50–100 Myr is
dominated by unknowable tectonics / biology / orbital chaos and is therefore
a **sensitivity experiment, not a prediction**.

---

## 1. Solar brightening track

Three supported tracks. Default = **blend** (§1.3). User MUST record which
track was used in run metadata (§6).

### 1.1 Gough (1981) analytic track — valid range limited

Formula (Gough 1981, *Solar Phys.* 74:21; widely requoted e.g. in Bahcall et
al. 2001):

```
L(t_age) = L0 / [ 1 + 0.4 * (1 - t_age / t0) ]
```

where

| Symbol | Meaning | Value |
|---|---|---|
| `t_age` | solar age since ZAMS (Gyr) | present `t0 = 4.57 Gyr` |
| `t0` | present solar age adopted | **4.57 Gyr** |
| `L0` | present solar luminosity | **3.846e26 W** (use model value consistently; IAU 2015 nominal `L☉ = 3.828e26 W` — record which constant) |
| `t_fut` | time after present (Gyr) | `t_age = 4.57 + t_fut` |

Equivalent forward form:

```
L(t_fut) / L0 = 1 / [ 1 - 0.4 * t_fut / 4.57 ] = 1 / (1 - 0.08753 * t_fut)
```

Reference values (forward form, exact to 3 d.p.):

| `t_fut` (Gyr) | `L/L0` (Gough) | dL/dt local (%/Gyr) |
|---|---|---|
| 0.0 (present) | 1.000 | ~8.8 |
| +0.1 | 1.009 | ~9.5 |
| +0.25 | 1.022 | ~10.7 |
| +0.5 | 1.046 | ~12.8 |
| +1.0 | 1.096 | ~19.0 |
| +1.5 | 1.151 | ~28.9 |
| +2.0 | 1.212 | ~45.6 |

Present-day rate ≈ **+8–10% per Gyr** (≈ +0.9% per 100 Myr). Consistent with
standard-model estimates (Bahcall et al. 2001; Sackmann et al. 1993).

**Valid range (hard rule):** Gough's fit was calibrated to main-sequence
homology for **solar age ≈ 0–6.5 Gyr (i.e. `t_fut ≤ ~2.0 Gyr`)**, and is
already degrading past `t_age ≈ 6 Gyr`. Implementations:

- `t_fut ≤ 1.0 Gyr`: Gough allowed standalone.
- `1.0 < t_fut ≤ 2.0 Gyr`: Gough allowed but MUST log
  `SOLAR_EXTRAPOLATION_WARNING`.
- `t_fut > 2.0 Gyr`: Gough **forbidden** standalone; use blend (§1.3) or
  tabulated stellar track. (Reason: post–~7 Gyr age the Sun's radius/Teff
  evolution departs from the homology scaling; full models brighten faster
  and heat the photosphere — see Sackmann, Boothroyd & Kraemer 1993, ApJ
  418:457.)

Do NOT use Gough backward past ZAMS (`t_age < 0`).

### 1.2 Bahcall blend table (Bahcall, Pinsonneault & Basu 2001, ApJ 555:990)

For `t_fut > ~0.5 Gyr` the implementation SHOULD blend/switch to a full
standard-solar-model tabulation. Reference table below is **rounded to 2
d.p. for protocol illustration only** — implementations MUST interpolate
the published electronic table (Bahcall et al. 2001, Table 3 / associated
data release), never these rounded numbers.

| Solar age (Gyr) | `t_fut` (Gyr) | `L/L0` ≈ | `R/R0` ≈ | `Teff` (K) ≈ |
|---|---|---|---|---|
| 4.57 (present) | 0.00 | 1.00 | 1.00 | 5777 |
| 5.0 | 0.43 | 1.05–1.06 | 1.03–1.04 | ~5790 |
| 6.0 | 1.43 | 1.17–1.18 | 1.11–1.13 | ~5820 |
| 7.0 | 2.43 | 1.30–1.33 | 1.22–1.25 | ~5850 |
| 8.0 | 3.43 | 1.46–1.50 | 1.37–1.42 | ~5880 |

(Envelopes reflect differences between Bahcall et al. 2001, Sackmann et
al. 1993, and Girardi et al. 2000 isochrones. Cite: Sackmann, Boothroyd &
Kraemer 1993; Girardi et al. 2000, A&AS 141:371.)

**Blend rule (default):**

```
w(t_fut) = smoothstep over t_fut in [0.5, 1.0] Gyr: 0 -> 1
L/L0 = (1-w) * L_Gough + w * L_Bahcall_interp
```

- Below 0.5 Gyr: pure Gough (tracks agree to <1%).
- 0.5–1.0 Gyr: linear/smooth cosine blend (no discontinuity in L or dL/dt
  larger than 0.5%/100 Myr).
- Above 1.0 Gyr: pure interpolated Bahcall table (log-linear in age).
- Past table end (~3.5 Gyr future / age 8 Gyr): STOP with
  `SOLAR_TABLE_EXHAUSTED` — do not linearly extrapolate. The 1-Gyr run
  (§4) never hits this, but longer experiments would.

Radius/Teff tracks are carried alongside luminosity for any diagnostic
that needs them (e.g. spectral weighting of albedo). Climate forcing uses
**luminosity only** unless the model explicitly documents a spectral
correction.

Insolation conversion (circular, fixed 1 AU unless §2 overrides):

```
S(t_fut)/S0 = (L(t_fut)/L0) * (a0/a)^2,  S0 = 1361 W/m^2 (record value used)
```

### 1.3 Track selection + user override format

Config block (YAML shown; JSON equivalent keys). One of
`gough1981 | bahcall2001-blend (DEFAULT) | custom-table | constant`.

```yaml
solar:
  model: bahcall2001-blend   # DEFAULT
  L0_W: 3.846e26             # record; alternative IAU-2015 3.828e26 allowed if global
  S0_Wm2: 1361.0
  t0_Gyr: 4.57
  blend_window_Gyr: [0.5, 1.0]
  # --- only for model: custom-table ---
  custom_table_csv: "input/solar/L_vs_age_Gyr.csv"  # columns: age_Gyr,L_L0,R_R0,Teff_K ; monotonic age; MUST span full run
  interpolation: "loglinear"   # loglinear | linear | cubic-monotone (no natural cubic)
  # --- only for model: constant (control runs) ---
  fixed_L_L0: 1.0
  # --- only for model: gough1981 ---
  allow_extrapolation_past_1Gyr: false   # if true, logs SOLAR_EXTRAPOLATION_WARNING each checkpoint
```

Validation rules:

1. `custom-table` MUST cover `[4.57, 4.57 + t_end]` with ≥1 point per
   0.25 Gyr; out-of-range = hard error, never silent clamp.
2. `constant` is for **control / stability runs only** (§4.2); any science
   run with `constant` MUST be suffixed `_NOFORCING_CONTROL`.
3. Every run manifest records: model name, table hash (SHA-256 of CSV or of
   vendored Bahcall table), `L0`, `S0`, and `L/L0` at `t_end`.
4. Time unit is Gyr (float64). No Myr/Gyr mixing inside the solar module —
   convert at the I/O boundary.

---

## 2. Next-supercontinent-cycle initial conditions

The solid-Earth boundary condition beyond ~50 Myr is unknowable; the
protocol therefore offers **two sanctioned options** (plus documented
variants). The run MUST declare exactly one.

### Option A — `continue-present` (default for ≤250 Myr legs)

Extrapolate present-day plate kinematics, then freeze.

| Parameter | Prescription |
|---|---|
| Plate velocities | Present-day field (e.g. MORVEL / NUVEL-1AKreemer-style, or model-native equivalent) held constant for first **50 Myr** of forward run |
| After 50 Myr | **Freeze** geography (no further drift). Rationale: linear extrapolation past ~50 Myr produces self-intersecting continents;แลนด์ errors compound faster than climate signal (cf. Scotese 2021 PALEOMAP review) |
| Land fraction | Fixed at model's present-day value (record; typically ~0.29) |
| Weatherability | Constant present-day calibration; no orogenic pulse |
| Use when | Stability runs (§4), any run where tectonic signal must be *minimized* |

### Option B — `pangea-proxima-like` assembly (for 250 Myr–1 Gyr legs)

A single idealized next-Pangea assembled per the Scotese "Pangea Proxima"
family (Scotese 2007+; PALEOMAP Project; see also Rogers & Santosh 2004;
Nield et al. 2023 *Nature Geosci.* "Pangea Ultima" mammalian-habitability
scenarios).

Assembly prescription (model-agnostic, grid-independent):

| Phase (forward time) | Geography rule |
|---|---|
| 0–50 Myr | As Option A (extrapolate then begin morph) |
| 50–200 Myr | Morph landmasses toward target supercontinent (great-circle interpolation of continental centroids; record interpolation weights per 10-Myr checkpoint) |
| 200–250 Myr | Full assembly: single supercontinent + superocean |
| 250 Myr–1 Gyr | **Hold assembled** (static). Optional ±10% land-fraction breakup sensitivity branch |

Target supercontinent parameters (defaults; override MUST be logged):

| Parameter | Default (Pangea-Proxima-like) | Allowed range / notes |
|---|---|---|
| Land fraction | 0.29 (conserved) | 0.26–0.32 if breakup branch enabled |
| Centroid latitude | ~equatorial–subtropical (mean abs. lat. 20–30°) | polar-cluster variant = separate declared option (Amasia-like) |
| Continental clustering | 1 supercontinent, interior-to-margin distance ~5,000–8,000 km | drives interior aridity |
| Interior aridity factor | ×1.5–2.0 runoff deficit vs zonal mean in cells >2,500 km from coast | after Nield et al. 2023; tune once, freeze |
| Silicate-weatherability multiplier | 0.7–0.9× present (large arid interior weathers slowly) | Franck et al. 1999-style; record value |
| Orography | One equatorial collisional belt (mean +1.5 km over belt cells) + passive margins elsewhere | no time-varying orogeny after 250 Myr |

### Named variants (declare explicitly; not silent tweaks)

| Variant key | Closure geometry | Source |
|---|---|---|
| `pangea-proxima` (B-default) | Atlantic closes; Americas dock E. Africa/Europe | Scotese PALEOMAP |
| `aurica-like` | Pacific closes; Americas+Asia merge, Atlantic opens | Duarte et al. 2018; Duarte, Schellart & Rosas 2016 |
| `amasia-like` | Continents cluster around N. pole (Arctic closure) | Hoffman 1999 "Wilson cycle" discussion; Mitchell et al. 2012 *Nature* 482:208 |
| `pangea-ultima-hot` | Proxima geometry + aridity factor at top of range (×2.0) — the Nield et al. 2023 high-heat endmember | Nield et al. 2023 |

Config block:

```yaml
tectonics:
  mode: continue-present   # or pangea-proxima-like
  variant: pangea-proxima  # only if mode is pangea-proxima-like
  freeze_after_Myr: 50     # continue-present only
  assembly_window_Myr: [50, 250]   # proxima-like only
  land_fraction: 0.29
  aridity_factor: 1.75
  weatherability_mult: 0.8
  random_seed: null        # MUST be null (no stochastic drift); any stochastic breakup model is out of scope
```

Rule: **no stochastic plate noise.** Two runs with identical config MUST be
bit-comparable to checkpoint precision (§4.3).

---

## 3. Forcing summary (what actually changes over 1 Gyr)

| Forcing | 0–0.25 Gyr | 0.25–1 Gyr | Notes |
|---|---|---|---|
| Solar `L/L0` | 1.00 → ~1.02 | → ~1.10 (Gough) / ~1.12 (Bahcall interp) | Dominant term; §1 |
| Geography | per §2 | frozen after 250 Myr | Secondary via albedo/weathering |
| `pCO2` baseline | model prognostic | prognostic; no prescribed ramp | Watch vs. carbonate-silicate response (Franck et al. 1999; Goddéris et al.) |
| Orbit / obliquity | fixed present (or fixed circular 1 AU) | fixed | Milankovitch unresolved at 10-Myr sampling; MUST document as fixed |
| Biology | fixed functional types | fixed | No evolution; flag as limitation in every output (§6) |

---

## 4. 1-Gyr stability run acceptance

A "1-Gyr stability run" = integration from present initial conditions to
`t_fut = +1.0 Gyr` under the declared solar + tectonics config, sampled
per §4.3. Pass = ALL of §4.1 + §4.2.

### 4.1 No-crash criteria (all must hold; any violation = FAIL)

| # | Criterion | Threshold / test | Rationale |
|---|---|---|---|
| C1 | No non-finite state | Zero `NaN/±Inf` in any prognostic field at any step | Hard integrity |
| C2 | No negative reservoirs | C, H₂O, ice, biomass pools ≥ 0 (allow −1e-12 relative floor for roundoff, clamped + counted) | Mass conservation sanity |
| C3 | Energy residual bounded | \|ASR − OLR − dH/dt\| ≤ **0.5 W/m²** RMS over trailing 1 Myr, ≤ **2.0 W/m²** instantaneous (except during documented7667 state-switch events) | Catches silent solver drift |
| C4 | Timestep floor | Adaptive dt never below **1% of nominal** for >1,000 consecutive steps; hitting floor = FAIL (not "slow pass") | Prevents Zeno-passing |
| C5 | Solver convergence | Implicit solver iterations ≤ cap at ≥99.9% of steps; non-converged steps ≤ 0.1%, each logged, none consecutive | — |
| C6 | Mass conservation | Total C + total H₂O column drift vs. sources−sinks ≤ **0.1%** cumulative (control run) / ≤ **1%** (forced run) | — |
| C7 | Watchdog discipline | Runaway/moist-greenhouse watchdog (§5) evaluated every step; never disabled; any `TERMINATE` trigger ends run as `TERMINATED_PHYSICAL` (counts as completed-run-with-limit, NOT a crash, but NOT a stability pass) | Distinguishes physics limit from bug |

Clamp counters (C2 roundoff clamps, C5 non-converged steps) MUST be in the
run ledger. More than 10⁴ clamps per 100 Myr = FAIL (masking instability).

### 4.2 Drift bounds

Evaluated on the **control twin** (same everything, `solar.model:
constant`, `fixed_L_L0: 1.0`) AND on the forced run. Numbers are
upper bounds on |end − start| of the trailing-10-Myr mean unless noted.

| Quantity | Control-run bound (constant Sun) | Forced-run bound (brightening Sun) | Check |
|---|---|---|---|
| Global-mean surf. temp. `T_s` | ≤ **0.5 K** per 100 Myr (≤5 K over 1 Gyr is FAIL even if per-century looks fine — no error-budget laundering) | No fixed cap (forced warming expected); instead **monotone + smooth**: no step >2 K per 10 Myr except at declared geography morph steps | Control catches drift; forced checks smoothness |
| TOA net flux (trailing-1-Myr mean) | ≤ **0.1 W/m²** abs. | ≤ **0.5 W/m²** abs. deviation from `dH/dt`-consistent value | — |
| Atmospheric C inventory | ≤ **2%** relative | ≤ **10%** relative excursion outside watchdog-flagged intervals | — |
| Ocean C / alkalinity | ≤ **2%** | ≤ **15%** | — |
| Sea level equiv. / ice volume | Bounded, no secular trend (Mann-Kendall p > 0.05 on 10-Myr series) | Same unless deglaciated-then-stable (document) | — |
| Stratospheric H₂O (control) | ≤ **10%** relative drift | Watch thresholds apply (§5) | Control must not creep into moist regime alone |

A forced run PASSES stability iff: C1–C6 hold, control twin passes its
column, and forced smoothness holds. `TERMINATED_PHYSICAL` (§5) is an
honorable non-pass: publishable as a limit experiment, not as stability.

### 4.3 Checkpoint sampling — every 10 Myr (100 checkpoints + t₀)

Cadence is **non-negotiable**: `t = 0, 10, 20, …, 1000 Myr` (101
checkpoints). No decimation, no "every 50 Myr to save disk."

Per checkpoint, store ALL of:

1. **Full restart state** (bit-exact, model-native; sufficient to resume).
2. **Diagnostic row** (CSV/Parquet, one row per checkpoint):
   `t_Myr, L_L0, S_Wm2, CO2_ppm, Ts_K, OLR_Wm2, ASR_Wm2, albedo, strat_H2O_vmr, ice_frac, land_frac, C_atm_PgC, C_ocean_PgC, watchdog_state`.
3. **State hash** (SHA-256 of restart file) in a `checkpoints_manifest.json`
   with `{index, t_Myr, sha256, watchdog_state, L_L0}`.
4. **2-D fields** (decimated to ≤1° or model-native coarse grid):
   `Ts, precipitation, ice mask, land mask` — needed to audit supercontinent
   interior heat.
5. **Log slice**: solver iters, dt, clamp counts, energy residual since last
   checkpoint.

Storage estimate note: full states dominate; if disk-limited, thin item 4
before anything else — NEVER thin items 1–3.

Determinism: re-running from checkpoint `N` to `N+1` MUST reproduce
checkpoint `N+1` hash. CI spot-checks at least 2 random segments.

---

## 5. Runaway-greenhouse / moist-greenhouse watch thresholds vs `L(t)`

Thresholds are on **stellar flux relative to present (`S/S0 = L/L0` at
1 AU)**, convertible to time via §1. Values below synthesize Kasting 1988;
Kopparapu et al. 2013 (ApJ 765:131) + 2014 erratum; Leconte et al. 2013
(*Nature* 504:268); Wolf & Toon 2015 (*JGR* 120:577); Goldblatt & Watson
2012; Franck, Bounama & von Bloh 2006. Bands reflect genuine model spread —
implementations MUST NOT collapse them to a single "doom date."

### 5.1 Flux-threshold ladder (1 AU, 1 M⊕; scale others per Kopparapu §5.3)

| Level | `S/S0` trigger (= `L/L0` at 1 AU) | Approx. Gough time* | Meaning |
|---|---|---|---|
| `NOMINAL` | < 1.01 | now → ~+0.1 Gyr | Inside moist-greenhouse uncertainty floor; normal ops |
| `WATCH_MOIST` | **≥ 1.01** | ~+0.1 Gyr | Enter Kopparapu moist-greenhouse band (1.014 for 1 M⊕ runaway-vs-moist ordering per Kopparapu 2013; Kasting 1988 gave ~1.10 for the water-loss limit — band spans both) |
| `WARNING_MOIST` | **≥ 1.06** | ~+0.6 Gyr | Above most 1-D moist-greenhouse onsets; 3-D onset (Leconte 2013; Wolf & Toon 2015 find ~1.10–1.15 for fast rotators) approaching |
| `WATCH_RUNAWAY` | **≥ 1.10** | ~+1.0 Gyr | Kopparapu runaway limit 1.107; Goldblatt OLR asymptote ~280–310 W/m² relevant here |
| `TERMINATE_RUNAWAY` | **≥ 1.20** (hard) **or** physical trigger below | ~+1.7 Gyr (Gough) — beyond the 1-Gyr window; physical trigger will usually fire first | Stop forcing solution; mark `TERMINATED_PHYSICAL` |

\* Times via Gough forward form; Bahcall blend shifts them ≤10%. The
protocol's 1-Gyr run (L/L0 → ~1.10) is therefore expected to live in
`WATCH_MOIST`→`WATCH_RUNAWAY` by its final checkpoints — that is *by
design* (watch the approach, do not cross into runaway inside the
acceptance run).

### 5.2 Physical (state-based) triggers — override flux ladder

Evaluate every step; FIRST trigger wins. Any `TERMINATE` ends the run as
`TERMINATED_PHYSICAL`.

| Trigger | Watch (log) | Warning (flag + checkpoint dump) | Terminate |
|---|---|---|---|
| Global-mean `T_s` | > **300 K** | > **315 K** | > **330 K** sustained 1 kyr (above human/mammalian habitability; Nield et al. 2023 discuss 40–50 °C continental interiors under Proxima-like + brightening) |
| Stratospheric H₂O VMR (cold-trap, ~1–10 hPa) | > **1×10⁻³** | > **3×10⁻³** (classical Kasting moist-greenhouse water-loss criterion) | > **1×10⁻²** sustained |
| OLR | flattening: dOLR/dT_s < **0.5 W/m²/K** over trailing 1 Myr | OLR within **10 W/m²** of model-calibrated Simpson–Nakajima asymptote (default **295 W/m²**; MUST record calibrated value, range 280–310 per Goldblatt) | OLR capped while `T_s` still rising for >10 kyr (runaway signature) |
| Ice / cloud | ice-free + tropical cloud deck collapse (document model metric) | — | — (warning only; not terminate) |

### 5.3 Mass / radius scaling (when applicable)

If the forward model is ever run for non-Earth mass, scale §5.1 per
Kopparapu et al. 2013 §4 / Kopparapu et al. 2014 (different `S_eff` per
stellar Teff and planet mass). For THIS protocol's Earth-only 1-Gyr run the
table values apply directly; the scaling paragraph exists so a future
implementer does not hard-code 1.01/1.10 as universal constants.

### 5.4 Watchdog behavior rules

1. Watchdog state (`NOMINAL | WATCH_MOIST | WARNING_MOIST |
   WATCH_RUNAWAY | TERMINATED_PHYSICAL`) is a first-class output: in every
   checkpoint row, every plot title/legend, every log line on transition.
2. Transitions are **sticky upward** within a run (no
   `WARNING → NOMINAL` without a `HYSTERESIS_OVERRIDE` entry signed by a
   human reviewer + physical justification, e.g. CO₂ drawdown).
3. On `WARNING_*` or above: dump an extra off-cadence full checkpoint
   immediately (does not replace the 10-Myr cadence).
4. On `TERMINATE`: stop the physics, write `termination_report.json`
   `{trigger, t_Myr, L_L0, Ts_K, strat_H2O, OLR, flux_level}`, and exit 0
   with status `TERMINATED_PHYSICAL` (exit non-zero is reserved for
   crashes/fails — never conflate physics limits with bugs).

---

## 6. SPECULATIVE watermarking points (mandatory)

Deep-future outputs MUST be identifiable as speculative at every level a
file can be opened, skimmed, or plotted. Minimum set — all required,
none waivable:

| # | Point | Requirement |
|---|---|---|
| W1 | Run manifest | `speculative: true`, `horizon_Myr`, `solar.model`, table hash, `tectonics.mode/variant`, watchdog config hash |
| W2 | File headers | Every CSV/Parquet/NetCDF: global attr `SPECULATIVE_DEEP_FUTURE = "t_fut > 50 Myr: sensitivity experiment, not a prediction"` |
| W3 | Filenames | Forward-run artifacts include `.speculative.` infix (e.g. `ts_1Gyr.speculative.csv`); controls `_NOFORCING_CONTROL` per §1.3 |
| W4 | Plots | Diagonal `SPECULATIVE — t > +50 Myr` banner on any axis extending past +50 Myr + caption sentence stating unknowable tectonics/biology |
| W5 | Logs / stdout tail | Run open AND close lines: `SPECULATIVE RUN — DO NOT CITE AS PREDICTION (horizon=…)` |
| W6 | Checkpoint manifest | Each `checkpoints_manifest.json` entry carries `speculative: (t_Myr > 50)` |
| W7 | Termination report | `termination_report.json` carries `speculative: true` + `limit_class: moist-greenhouse | runaway-greenhouse | none(stability-pass)` |
| W8 | Downstream docs | Any doc/figure reusing these numbers MUST propagate the watermark (copy-paste of a bare number without `SPECULATIVE` = defect) |

Spot-check: `grep -rL SPECULATIVE <run-output-dir>` MUST return empty
(i.e. every file mentions it) — except raw binary restart dumps, which are
covered by W6 instead.

---

## 7. Minimal run matrix (suggested, not mandatory)

| Run | Solar | Tectonics | Purpose | Expected outcome |
|---|---|---|---|---|
| R0 control | `constant` 1.0 | `continue-present` frozen | Stability gate (§4.2 control column) | PASS required before R1–R3 count |
| R1 reference | `bahcall2001-blend` | `continue-present` | Solar-only 1-Gyr response | Ends in `WATCH_MOIST`, no terminate |
| R2 proxima | `bahcall2001-blend` | `pangea-proxima-like` | Tectonic + solar coupling; interior heat (cf. Nield 2023) | Warmer than R1; possible `WARNING_MOIST` late |
| R3 hot branch | `gough1981` (logged) or blend + `pangea-ultima-hot` | Upper-bound sensitivity | Upper envelope; terminate-allowed |

---

## 8. Cited sources (protocol basis — consult originals before implementing)

- Gough, D. O. 1981, *Solar Phys.* 74:21 — analytic `L(t)` homology fit.
- Bahcall, J. N., Pinsonneault, M. H. & Basu, S. 2001, ApJ 555:990 —
  standard solar model + electronic track tables (blend target).
- Sackmann, I.-J., Boothroyd, A. I. & Kraemer, K. E. 1993, ApJ 418:457 —
 ante- and post-main-sequence evolution; why Gough expires past ~2 Gyr
  future.
- Girardi, L. et al. 2000, A&AS 141:371 — isochrone cross-check on L/R/Teff.
- Kasting, J. F. 1988, *Icarus* 74:472 — runaway/moist greenhouse 1-D
  limits (water-loss ~1.10 S0 in original formulation).
- Kopparapu, R. K. et al. 2013, ApJ 765:131 (+ 2014 erratum) —
  `S_eff` moist-greenhouse 1.014 / runaway 1.107 for 1 M⊕ around Sun.
- Leconte, J. et al. 2013, *Nature* 504:268 — 3-D runaway onset above 1-D.
- Wolf, E. T. & Toon, O. B. 2015, *JGR* 120:577 — 3-D moist-greenhouse
  ~1.10–1.15; rotation dependence.
- Goldblatt, C. & Watson, A. J. 2012 (and Goldblatt et al.) — OLR
  Simpson–Nakajima asymptote ~280–310 W/m².
- Franck, S., Bounama, C. & von Bloh, W. (1999–2006 series) —
  carbonate-silicate + brightening biosphere-lifespan framing.
- Scotese, C. R., PALEOMAP Project (2007–, updated 2021) — Pangea Proxima
  assembly scenario; present-kinematics baseline.
- Mitchell, R. N. et al. 2012, *Nature* 482:208 — Amasia alternative.
- Duarte, J. C., Schellart, W. P. & Rosas, F. M. 2016; Duarte et al. 2018
  — Aurica alternative (Pacific closure).
- Nield, E. et al. 2023, *Nature Geosci.* — Pangea Ultima heat endmember
  (continental-interior mammalian habitability under next supercontinent).
- Rogers, J. J. W. & Santosh, M. 2004, *Continents and Supercontinents* —
  supercontinent-cycle context.

---

*End of scratch draft. Next step (not this file): implement solar module +
watchdog + checkpoint writer against this protocol, then run R0.*
