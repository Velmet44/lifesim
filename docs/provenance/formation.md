# Provenance: formation

## Equations (M1)
- Accretion window [−4.56, −4.45] Ga: per-cell relaxation toward magma-ocean
  target MAGMA_K = 1800 K (step-scaled rate); Theia scripted injection at
  4.51 Ga ± seed jitter (20 Myr × `theia_jitter`), one-step +400·E pulse
  (E ∈ [0.8, 1.2] from seed hash).
- Solidification [−4.45, −4.43] Ga: target ramps 1800 → 800 K across the
  window; first global mean < 900 K records `totals.solidified_yr` + event log.
- Post-window (−4.43, −4.0] Ga: deterministic protocrust relief
  `1500·sin(3λ)·cos(2φ) − 1000` m; pCO₂ held at volatile inventory
  20000·volatile_H2O ppm (geochem era target); dormant after −4.0 Ga.
- Faint-young-sun coupling (spec §23.1): atmos target carries
  ΔT = 6.5·ln(C/280) (≈ +28 K at 20000 ppm, ≡0 at preindustrial);
  M3 replaces this with the band radiation scheme.

## Key params + priors
- `theia_jitter = 0.1` (±1 Myr timing at default), `volatile_H2O = 1.0`.
  ([formation] in lifesim.toml; hashed into outputs.)
- SOLIDIFY_MEAN_K = 900 K detection; VOLATILE_CO2_PPM = 20000
  (spec §6.7 Archean steady-state band 1e3–1e5 ppm).

## Citations
- Canonical magma-ocean solidification ~4.43 Ga (checkpoint; cf. spec §16 T1).
- Walker-Urey-WHK weathering context for the volatile inventory (see geochem).
- Theia giant-impact hypothesis (Hartmann & Davis 1975; Cameron & Ward 1976).

## Validation
- F1: Tm > 1800 K at 4.54 Ga (±200 K).
- F2: solidification detected ~4.43 Ga (measured −4.4319 Ga), Ts 400–900 K.
- F3: liquid-water ocean fraction > 30% by 4.30 Ga (measured 0.77).
  (`crates/earth-cli/tests/m1.rs`, `crates/formation/tests/m1_formation.rs`)
- See spec.md §16 checkpoint table.
