# Digital-Twin Present-Mode Initializer — Procedure Spec (Scratch Draft)

**Scope:** Initialize a present-day digital twin from observations/reanalyses onto the L2 icosphere grid. Output is a restart + prescription bundle that feeds spin-up and then H-a / H-b / H-c hindcasts. No code here — procedure only.

**Target grid (L2):** icosahedral-hexagonal grid, subdivision level 2 refinement over base icosahedron (hereafter L2; ~1.0° nominal, ~110 km cell spacing, N_cells ≈ 10 242 + 12 pentagons). All fields cell-centered; fluxes face-centered where needed. Vertical: 32 atm levels (hybrid sigma-p to 1 hPa) + 20 ocean levels (z*), 6 soil layers, 3 ice categories. All remaps target L2 cell means.

**Reference epoch:** Present-mode = 1995–2014 climatology anchor, 2015-01-01 restart date. All forcings version-pinned (see §3).

---

## 1. Input inventory (fetch + checksum)

| # | Dataset | Version / DOI | Variables used | Native grid | Role |
|---|---------|---------------|----------------|-------------|------|
| 1 | ERA5 (Hersbach et al.) | CDS v2 / DOI 10.24381/cds.adbb2d47, hourly 1995–2014 | T, q, u, v, wspd, mslp, SST, SIC, soil T/q layers 1–4, snow depth/SWE, 2m T/d, surface fluxes | 0.25° lat-lon, 37 plev + surface | Atmos/land/ocean-seaice IC + climatology |
| 2 | ERA5-Land (gap fill) | grib v1.0 | soil moisture layers, snow, runoff | 0.1° | Land fallback where ERA5 land missing |
| 3 | ESA CCI Land Cover | v2.1.1 (1992–2015 maps; use 2010 epoch) | 22-class LC, forest fraction | 300 m | PFT mapping |
| 4 | ESA CCI Biomass | v4.0 (2010 AGB) | above-ground biomass Mg/ha | 100 m | Veg carbon pool init |
| 5 | ESA OC-CCI Ocean Colour | v6.0 monthly Chl-a 1998–2014 | surface Chl-a | 4 km | Ocean BGC / NPZD init |
| 6 | World Ocean Atlas | WOA23 T/S climatology | T, S 3-D | 0.25° × 102 levels | Ocean interior below ERA5/SST skin |
| 7 | BedMachine / GEBCO | GEBCO_2023 + BedMachine v3 polar | bathymetry, land elevation, ice mask | 15 arc-sec / 500 m polar | Mask reconciliation referee |
| 8 | GHG concentrations | Meinshausen et al. CMIP6 v4.1.2 (2017 update) | CO₂, CH₄, N₂O, 43 halocarbons annual global + lat gradient | global annual | Prescription table G1 |
| 9 | Volcanic SAOD | eVolv2k v3 (Toohey & Sigl 2017) | stratospheric AOD 550 nm, r_eff, lat-height | monthly, 90S–90N, 0–40 km | Prescription table V1 |
| 10 | Solar irradiance | SATIRE-TS v2.1 (Krivova/Wu) + SOLID composite | TSI, SSI 115–3000 nm daily | spectral, 1 nm bins | Prescription table S1 |
| 11 | Aerosols | MACv2 (Kinne 2019) + CMIP6 BB monthly | AOD550 fine/coarse, SSA, asym, BC/OC/SO₄ mass | 1° monthly climatology + transient BB | Prescription table A1 |
| 12 | Land-use / N-dep | LUH2 v2h (2016) + JAMF N-dep v2 | crop/pasture/urban fractions, N-dep | 0.25° annual | Land-use init table L1 |

Store all raw files under `init/raw/<source>_<version>/` with SHA256 manifest. Reject run if any checksum mismatches.

---

## 2. ERA5 climatology → L2 icosphere remap procedure

### Step 1 — Build 1995–2014 climatologies on native grid
1.1. Aggregate ERA5 hourly → daily means (T, q, u, v, mslp), then → monthly means per year, then → 20-year monthly climatology (12 × lat × lon × lev). Keep also 6-hourly 2014 full year for transient eddy variance / storm-track check.
1.2. Separate products:
  - (a) Atmosphere 3-D: T, q, u, v on 37 plevs + surface (2mT, 2md, 10m u/v, mslp, TCWV).
  - (b) Ocean skin: SST (daily), SIC (daily), wave height (monthly, diagnostic only).
  - (c) Land: soil temperature layers 1–4, volumetric soil water layers 1–4, snow depth, SWE, skin T.
1.3. Deseasonalize QC: flag any month/grid-point where 1995–2014 σ = 0 (dead sensor/constant lake mask) or |2014 anomaly| > 5σ; write to `qc_era5_native.nc`, do not fill yet.

### Step 2 — Generate conservative remap weights (once, cache)
2.1. Build SCRIP descriptors for source (0.25° Gaussian-rectilinear, cyclic lon) and destination (L2 icosphere polygons, exact spherical areas via Girard excess).
2.2. Compute first-order conservative weights with ESMF/SCRIP `conserve` (Jones 1999 area-weighted overlap). Destination cell value = Σ_src w_ij · src_j, Σ_j w_ij = A_dest,i / R² within 1e-12.
2.3. Compute second set of weights for intensive vs. extensive quantities:
  - Intensive (T, q, SST, SIC): area-weighted mean (normalize by ocean/land fraction as in Step 3).
  - Extensive (SWE mass, soil water column, precipitable water): area-integral conserving, units kg/cell.
2.4. Store `weights_era5r025_to_L2_conserve.nc` + `weights_L2_area.nc` (cell areas, corner lats/lons). Verify global integrals: total atmospheric dry mass conserved to <0.01%, total water column to <0.05% between native and L2 before masking (report in log).

### Step 3 — Land–sea mask reconciliation (critical)
Native ERA5 LSM (0/1 fractional at 0.25°) and L2 target mask disagree at coasts. Procedure:
3.1. Fix target L2 master mask first from GEBCO_2023 + BedMachine referee at 1 km → majority-area vote per L2 cell. Classes: ocean / land / lake / grounded-ice / floating-ice-shelf. Threshold: cell is land iff land-area fraction ≥ 0.5; lakes resolved iff lake fraction ≥ 0.5 and cell elevation < 3000 m and not ocean-connected. Freeze as `L2_master_mask_v1.nc` — never recomputed downstream.
3.2. Remap ERA5 fields twice: once masked with source-land-only weights, once with source-ocean-only weights (i.e., renormalize w_ij over src land cells and src ocean cells separately). This prevents ocean SST bleeding into coastal land cells and vice versa.
3.3. Assign per L2 cell by master mask:
  - Ocean cells: take ocean-only remap for SST/SIC; discard land remap.
  - Land cells: take land-only remap for soil/snow/skin; SST undefined → NaN.
  - Lake cells: SST from ERA5 lake-mixed-layer where available, else area-weighted 2mT − 1.5 K fallback, flagged `lake_fill=1`.
3.4. Coastal residual check: any L2 cell where |land-frac_native_remapped − land-frac_master| > 0.25 gets flagged `coast_fix`. For those (~3–5% of cells), replace 3-D atmospheric profiles by nearest all-land or all-ocean donor profile of matching class within 500 km (preserve class means, avoid step discontinuity).
3.5. Antarctica/Greenland: override ERA5 skin/snow with BedMachine ice mask; set ice-sheet cells to `landice` class, soil moisture → NaN, SWE → ERA5 SWE capped at 5000 kg m⁻².
3.6. Acceptance: global land area within 0.5% of GEBCO referee; global ocean area within 0.3%; number of `coast_fix` cells < 6% of total; no NaN in required fields except sanctioned (soil under ice, SST over land).

### Step 4 — Vertical interpolation to model levels
4.1. Atmosphere: log-pressure interpolation of T/q/u/v from 37 ERA5 plevs → 32 hybrid levels; relative humidity conserved (recompute q from RH + Clausius–Clapeyron on target levels, not direct q interp). Surface fields (mslp, 2mT) diagnostically downscaled with model orography lapse-rate correction (−6.5 K km⁻¹ × Δz).
4.2. Ocean: merge SST skin (top 10 m) + WOA23 interior below 10 m with linear blend 10–30 m. Remap WOA23 T/S conservatively in horizontal, linearly in depth to 20 z* levels. Salinity floor 2 PSU in Baltic/Arctic river mouths; flag `S_floor`.
4.3. Land subsurface: ERA5 4 soil layers → 6 model layers by mass-conserving spline (total column heat + water conserved to 0.1%); snow → single bulk layer + fractional cover via SWE–cover curve (cover = tanh(SWE/20)).

### Step 5 — Write IC bundle
`ic_present_L2_20150101.nc`: all prognostic fields + `mask`, `orog`, `cell_area`, `remap Provenance` (weights file hash, ERA5 CDS request hash, master-mask version). Plus `ic_clim12_L2.nc` (12-month L2 climatology for anomaly initialization option).

---

## 3. GHG / aerosol / solar / volcanic prescription tables (version-pinned)

All transient 1850–2014 (hindcast) + 2015 fixed present values. Model reads annual/global + prescribed meridional/vertical structure — never hard-coded.

### Table G1 — GHGs (Meinshausen CMIP6 v4.1.2)
| Species | 2015 value | 1850 value | Time resolution | Spatialization | Notes |
|---------|-----------|-----------|-----------------|----------------|-------|
| CO₂ | 400.8 ppm | 284.3 ppm | annual global + 4-lat-box gradient | well-mixed + NH+2 ppm seasonal pump | Seasonal cycle amplitude 6 ppm NH, imposed analytically |
| CH₄ | 1834 ppb | 808 ppb | annual global + lat gradient file | 12-lat-band file `ch4_latgrad.nc` | Wetland/OH not interactive in present-mode |
| N₂O | 328.7 ppb | 273 ppb | annual global uniform | uniform | |
| CFC-11 / CFC-12 / 40 others | per `ghg_CMIP6_v4.1.2.nc` | per file | annual | uniform | Full 43-gas table read; aggregated CFC-11-eq for radiation fast path |
Provenance file: `presc_GHG_Meinshausen-v4.1.2_1850-2015.nc`. Checksum pinned; 2015 fixed at Jan value (no intra-year drift).

### Table A1 — Aerosols (MACv2 + CMIP6 BB)
| Component | Source | Resolution | Optical props |
|-----------|--------|-----------|---------------|
| Anthropogenic AOD550 fine/coarse, SSA, g | MACv2 climatology (Kinne 2019) + linear 1850→2015 scale | monthly 1° → conservatively remapped to L2 | Per-band Mie table `aero_optics_MACv2.nc` (14 SW + 16 LW) |
| Biomass-burning BC/OC | CMIP6 BB (van Marle/Gidden) monthly transient 1850–2014 | monthly L2 | Mass → optics via same table |
| Dust / sea-salt | Interactive emission, MACv2 2015 clim as spin-up target | monthly | Tuned to MACv2 AOD within ±15% zonal |
| Volcanic tropospheric SO₂ | OMI/HTAP climatology constant | annual | Background only; stratospheric in V1 |
Rule: present-mode uses *prescribed* anthropogenic + BB (no interactive chemistry feedback onto radiation); dust/sea-salt interactive but nudged to MACv2 during spin-up then freed for hindcasts.

### Table S1 — Solar (SATIRE-TS v2.1)
| Quantity | 2015 (cycle 24 declining) | 1850–2014 |
|----------|--------------------------|-----------|
| TSI | 1361.0 W m⁻² (Jan 2015 daily) | Daily reconstruction `ssi_SATIRE-TS_v2.1_daily.nc`, 115–3000 nm |
| SSI bins | 8-band model-mapped file | Same, spectrally resolved; UV 200–320 nm drives ozone top |
| F10.7 proxy | 129 sfu | For thermosphere drag diagnostic only |
Hindcasts use daily SSI; climatology runs may use annual-mean file `ssi_annual.nc` (document which).

### Table V1 — Volcanic (eVolv2k v3, Toohey & Sigl 2017)
| Eruption window | SAOD550 peak (global) | Prescription |
|-----------------|----------------------|--------------|
| Pinatubo Jun 1991 | 0.15 | Monthly lat-height SAOD + r_eff 0.35→0.55 µm, 18–25 km |
| El Chichón 1982 | 0.10 | Same structure |
| Agung 1963 | 0.09 | Same |
| All others VEI≥4 1850–2014 | per eVolv2k file | `saod_eVolv2k-v3_monthly.nc`, background 0.002 when quiet |
| 2015 present anchor | 0.003 (Calbuco Apr 2015) | Fixed; no eruption during 2015 unless H-c specifies |
Radiation ingests SAOD + r_eff → per-band extinction; heating-rate check in spin-up Step 8.

All four tables + versions recorded in `presc_provenance.yaml` (file, version, DOI, SHA256, time span). Model aborts if version string mismatches.

---

## 4. Land-use / biomass / ocean-color init mapping

### Table L1 — Land (ESA CCI + LUH2)
| Step | Source → target | Rule |
|------|----------------|------|
| L1.1 PFT crosswalk | ESA CCI 22 classes → 14 model PFTs (9 veg + crop/pasture/urban/bare/ice) | Map file `xwalk_CCI22_to_PFT14_v1.csv` (e.g., CCI 50/60/70 broadleaf → PFT broadleaf-evergreen/deciduous by latitude ±23.5°; CCI 10/11/12 cropland → C3/C4 crop by LUH2 C4 fraction) |
| L1.2 Fractions | 300 m CCI → fractional cover per L2 cell | Conservative area tally; enforce ΣPFT = 1 over land; crop/pasture overwritten by LUH2 2015 fractions where LUH2 > CCI by >0.1 (transient land-use wins) |
| L1.3 Veg carbon | ESA CCI Biomass v4.0 AGB 2010 → leaf/stem/root pools | AGB → total veg C via root:shoot 0.26 (forest) / 0.8 (grass) / 1.2 (shrub); leaf = 8% forest / 35% grass of total; spin-up adjusts, so ±30% tolerance OK; fill voids (desert/ice) with 0 + flag |
| L1.4 Soil carbon | HWSD v1.2 SOC top-1m + NCSCD polar | 3-pool RothC split 60/30/10 fast/slow/passive; permafrost cells (T_ann < 0 °C) ×1.8 passive multiplier |
| L1.5 N deposition | JAMF v2 2015 | Annual rate per L2 cell, ثابت for present-mode |
QC: global forest area 39–43 Mkm²; crop+pasture 48–52 Mkm²; veg C 450–650 PgC; soil C 1400–1700 PgC. Outside → halt.

### Table O1 — Ocean BGC (ESA OC-CCI v6.0)
| Step | Rule |
|------|------|
| O1.1 Surface Chl | 1998–2014 monthly Chl-a climatology → L2 (conservative); log-mean, not arithmetic (Chl lognormal) |
| O1.2 NPZD init | Phytoplankton C = 40 × Chl (C:Chl mass), zooplankton = 0.3 × phyto, detritus = 0.5 × phyto, nutrients (N/P/Si/Fe) from WOA + GLODAPv2 nitrate/phosphate/silicate climatology; iron from PISCES dust-deposition climatology constant |
| O1.3 Ice/ocean mask | Chl under SIC > 15% → set to polar winter value 0.05 mg m⁻³ + flag `ice_chl_fill` |
QC: global mean surface Chl 0.15–0.25 mg m⁻³; no negative nutrients.

---

## 5. Spin-up protocol (model-years + drift thresholds to accept init)

Goal: equilibrium present-day restart with drift small vs. interannual variability. Two phases.

### Phase A — Component equilibration (parallel, fixed climate)
- A1. Land-only 50 model-years: force with L2 12-month climatology (`ic_clim12_L2.nc`) + fixed 2015 CO₂/GHG/aerosol/solar, cycling. Target: global NEP |drift| < 0.1 PgC yr⁻¹ over last 10 yr; soil C drift < 0.5 PgC yr⁻¹; runoff vs. ERA5 climatology within ±10%.
- A2. Ocean-only (incl. sea-ice + BGC) 200 model-years: CORE-JRA repeat-year forcing derived from ERA5 climatology + WOA relax (SSS restore 90-day, SST relax 30-day first 50 yr, then free next 150 yr). Target: AMOC at 26°N 15–19 Sv; global OHC 0–2000 m drift < 0.05 W m⁻² equivalent (<0.08 × 10²³ J decade⁻¹); sea-ice extent annual mean NH 11–13 Mkm², SH 11–13 Mkm²; surface Chl seasonal phase correlation > 0.7 vs OC-CCI.
- A3. Atmosphere-only (AMIP-style) 10 model-years: prescribed SST/SIC climatology + 2015 forcings. Target: global TOA net < ±1.0 W m⁻² (tunable), precipitation 2.6–2.8 mm day⁻¹, 200-hPa jet position ±3° vs ERA5.

### Phase B — Coupled equilibration
- B1. Coupled 100 model-years with 2015 fixed forcings (no transient trend). Start from A-combined restart.
- B2. Drift acceptance thresholds (all must pass over final 30-yr window, linear trend):
  1. TOA net imbalance drift < 0.05 W m⁻² decade⁻¹ AND absolute mean within ±0.5 W m⁻².
  2. Global 2mT drift < 0.02 K decade⁻¹.
  3. OHC 0–2000 m drift < 0.1 W m⁻² equivalent.
  4. AMOC drift < 0.3 Sv decade⁻¹.
  5. Sea-ice extent trend < 0.2 Mkm² decade⁻¹ each hemisphere.
  6. Land C (veg+soil) drift < 0.2 PgC yr⁻¹; ocean DIC drift < 0.1 PgC yr⁻¹.
  7. Stratospheric volcanic heating check: Pinatubo-test 2-yr branch reproduces ≥1.5 K tropical lower-stratosphere warming (validates V1 optics wiring).
- B3. On fail: extend in 50-yr blocks (max 300 yr total); if TOA or OHC still failing, retune (cloud/sea-salt) — do not accept by widening thresholds. Record tuning deltas in `spinup_tuning.log`.
- B4. Accepted restart: `restart_present_20150101_L2_spinup<N>yr.nc` + `spinup_drift_report.yaml` (all 7 metrics + plots). This is the H-series branch point.

Total nominal cost: 50 (land, cheap) + 200 (ocean) + 10 (atm) + 100 (coupled) ≈ 360 component-years, ~100 coupled-years critical path.

---

## 6. H-a / H-b / H-c hindcast setup wiring

All three branch from the accepted `restart_present_20150101_L2_spinup` (or its 1850 back-cast twin where noted). Prescription tables from §3 wired per experiment.

| Experiment | Purpose | Init branch | Forcing wiring | Length / ensemble | Pass criteria |
|------------|---------|-------------|----------------|-------------------|---------------|
| H-a Historical transient | 1850–2014 reproduction | Back-cast: same L2 restart, swap 2015 → 1850 GHG/aerosol/LUH2 (Table G1/A1/L1 1850 values), 20-yr ocean relax to 1850 climatology, then forward transient | Transient GHG (G1 annual), transient BB + MACv2→1850-scaled anthro (A1), daily SATIRE SSI (S1), eVolv2k SAOD (V1), LUH2 annual land-use | 165 yr × 3 members (lagged Jan/Feb/Mar 1850 ocean IC by ±0.01 K noise) | GMST anomaly 1850–2014 correlation > 0.85 vs HadCRUT5; Pinatubo cooling −0.3±0.15 K; AMOC decline < 3 Sv; sea-ice retreat timing ±10 yr |
| H-b AMIP / initialized seasonal-decadal | Atmosphere fidelity + initialized skill | Direct from 2015 restart for AMIP (1979–2014 prescribed SST/SIC) and from H-a 1960/1980/2000 checkpoints for initialized runs | AMIP: observed SST/SIC (HadISST), forcings as H-a; initialized: full-field IC from H-a + ERA5 anomaly blend (weight 0.5), then free | AMIP 36 yr × 1; initialized: 10-yr forecasts from 5 start dates × 5 members | AMIP precip pattern RMSE < 15% vs GPCP; initialized ACC(5-yr-mean GMST) > 0.6; ENSO Niño3.4 ACC(12-mo) > 0.5 |
| H-c Single-forcing / attribution | GHG-only, aerosol-only, solar+volc-only, natural-only | Branch from H-a 1850 restart (identical IC, bit-reproducible) | Ghost-file method: all forcings read every step, non-active set held at 1850 (GHG-only → aerosols/solar/volc at 1850; nat-only → GHG/aerosols at 1850, solar+volc transient) + per-run `forcing_switch.yaml` | 165 yr × 3 members each (4 bundles = 12 runs) | GHG-only 2014 warming 1.0–1.5 K; aerosol-only cooling −0.2 to −0.6 K; natural-only trend ≈ 0 ± 0.15 K century⁻¹; residual (all − sum singles) < 0.2 K (linearity check) |

Wiring details:
6.1. Forcing router: single `forcing_router.nc` namelist per run pointing to G1/A1/S1/V1/L1 file + version strings; H-c ghost files are symlinks to 1850 slices (no duplication). Router checksum logged per run.
6.2. IC blending for H-b initialized: `IC = H-a_checkpoint + 0.5 × (ERA5_L2_anomaly_observed − H-a_clim_anomaly)`, atmosphere/land full weight, ocean top-500 m 0.5 weight, deep ocean 0.1 weight (avoid shock). 30-day SST-relax taper after init.
6.3. Output: monthly L2 means + daily extremes subset; H-a/H-c also store forcing-response diagnostics (ERF per agent via double-call radiation, annual).
6.4. Failure routing: any H-run failing its pass criteria does not re-spin; file issue against forcing table version first (most common: aerosol scaling, eVolv2k background), then IC blend weight, then model physics — in that order.

---

## 7. File manifest (init bundle)

```
init/
  raw/<source>_<version>/ + SHA256SUMS
  weights_era5r025_to_L2_conserve.nc
  L2_master_mask_v1.nc
  ic_present_L2_20150101.nc + ic_clim12_L2.nc
  presc_GHG_Meinshausen-v4.1.2_1850-2015.nc
  aero_MACv2_clim.nc + BB_CMIP6_transient.nc
  ssi_SATIRE-TS_v2.1_daily.nc
  saod_eVolv2k-v3_monthly.nc
  xwalk_CCI22_to_PFT14_v1.csv + landfrac_LUH2_2015_L2.nc
  chl_OCCCI-v6.0_L2.nc
  presc_provenance.yaml
  spinup_drift_report.yaml
  restart_present_20150101_L2_spinup<N>yr.nc
  forcing_router_<Ha|Hb|Hc>_<member>.nc + forcing_switch.yaml (H-c)
```

Sign-off: init accepted only when §§2.3.6 + 4 + 5-B2 + 6 pass criteria all green. Any dataset version change → new `L2_master_mask` or `presc_provenance` revision, full §5-B re-spin required.
