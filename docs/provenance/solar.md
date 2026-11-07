# Provenance: solar

## Equations (pinned, M1)
- Luminosity, Gough (1981): `L(τ) = L⊙ / (1 + 0.40·(1 − τ/τ⊙))`, τ⊙ = 4.57 Gyr,
  L⊙ = 3.828e26 W (TSI₀ = 1361 W/m²).
  Lookback: `L(t_lb) = L⊙ / (1 + 0.40·t_lb/τ⊙)`. Valid τ ∈ [0, 8] Gyr;
  future past +2 Gyr is a hard error (blend to Bahcall+2001 table instead).
  Present rate dL/dt|₀ = 0.40/τ⊙ = +0.0875 L⊙/Gyr.
- EUV/XUV: `F(t) = F₀·(t/4.57 Gyr)^(−1.2)`, t ≥ 0.1 Gyr (Ribas+2005; Tu+2015),
  saturating at 100× modern below 0.1 Gyr. Feeds H-escape (geochem) +
  magnetopause (interior) from M4.
- Orbit lite (Berger 1978 / Laskar+2004 frequencies): eccentricity
  0.028 + 95/125/400 kyr terms; obliquity 23.3 ± 1.2° at 41 kyr, clamped to
  [22.1, 24.5]°; precession 19/23 kyr longitude of perihelion.
- Lunar: recession 3.83 cm/yr (LLR-anchored, linear M1 simplification);
  day length anchors 4.5 Ga → 6.5 h / 2.45 Ga → 17 h / present → 24 h
  (angular-momentum conservation); tidal mixing κ ∝ a^(−3), 1.5 TW present.

## Key params + priors
- τ⊙ = 4.57 Gyr (fixed), TSI₀ = 1361.0 W/m², L⊙ = 3.828e26 W.
- EUV exponent −1.2 ± 0.2 (test pins −1.2; spread lands with M4 escape coupling).

## Citations
- Gough, D. O. 1981, *Solar Phys.* 74:21.
- Bahcall, J. N., Pinsonneault, M. H. & Basu, S. 2001, ApJ 555:990 (blend target).
- Ribas, I. et al. 2005; Tu, L. et al. 2015 (EUV decay).
- Berger, A. 1978; Laskar, J. et al. 2004 (orbital solution).
- Munk, W. & Wunsch, C. (tidal dissipation 1–2 TW); LLR lunar recession.

## Validation
- L(4.38Ga) = 0.728 ± 0.01 L⊙; L(2.5Ga) = 0.821 ± 0.015; dL/dt|₀ = +0.087 ± 0.01.
  (`crates/solar/tests/m1_solar.rs`)
- Orbit component periods ±1% by zero-crossing measurement (M1 gate).
- See spec.md §16 checkpoint table.
