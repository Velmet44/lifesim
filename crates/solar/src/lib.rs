//! Solar / astro forcing (spec §6.1, M1).
//!
//! Pinned forms:
//! - Luminosity: Gough (1981), `L(τ) = L⊙ / (1 + 0.40·(1 − τ/τ⊙))`, τ⊙ = 4.57 Gyr.
//!   Lookback: `L(t_lb) = L⊙ / (1 + 0.40·t_lb/τ⊙)`. Valid τ ∈ [0, 8] Gyr;
//!   future past +2 Gyr is a hard error (blend to Bahcall+2001 table instead,
//!   never linear extrapolation — see docs/deep-future.md §1).
//! - EUV/XUV: `F(t) = F₀·(t/4.57)^(−1.2)`, t ≥ 0.1 Gyr (Ribas+2005; Tu+2015),
//!   saturated at 100× modern below 0.1 Gyr.
//! - Orbit lite: Berger (1978) / Laskar+2004 frequencies — ecc 95/125/400 kyr,
//!   obliquity 41 kyr in [22.1, 24.5]°, precession 19/23 kyr.
//! - Lunar: recession anchored to LLR 3.83 cm/yr; day length from
//!   angular-momentum anchors (Hadean 5–8 h; 2.45 Ga 16–18 h; present 24 h).

use earth_core::{Diagnostics, FieldId, RngStream, SimResult, SimError, Subsystem, World};

pub const TAU_SUN_GYR: f64 = 4.57;
pub const L_SUN_W: f64 = 3.828e26;
pub const TSI0_W_M2: f64 = 1361.0;
/// Nominal rate dL/dt|₀ in L⊙/Gyr (spec check: +0.087 ± 0.01).
pub const DLDT0_LSUN_PER_GYR: f64 = 0.40 / TAU_SUN_GYR; // 0.08753

/// Solar age in Gyr since ZAMS for a simulation year (negative = past).
pub fn solar_age_gyr(t_yr: f64) -> f64 {
    TAU_SUN_GYR + t_yr / 1e9
}

/// Gough (1981) forward form: `L(t_fut)/L0 = 1 / (1 − 0.08753·t_fut)`.
/// Exact pinned form; valid t_fut ≤ ~2 Gyr (see `luminosity_ratio_at_age`).
pub fn gough_luminosity_ratio(t_fut_gyr: f64) -> f64 {
    1.0 / (1.0 - (0.40 / TAU_SUN_GYR) * t_fut_gyr)
}

/// Lookback form: `L(t_lb)/L0 = 1 / (1 + 0.40·t_lb/τ⊙)`.
pub fn gough_lookback_ratio(t_lb_gyr: f64) -> f64 {
    1.0 / (1.0 + 0.40 * t_lb_gyr / TAU_SUN_GYR)
}

/// Luminosity for a solar age with range discipline.
/// Returns `(L/L0, extrapolated_flag)`. Errors past +2 Gyr future
/// (caller must switch to the Bahcall+2001 table; never extrapolate).
pub fn luminosity_ratio_at_age(t_age_gyr: f64) -> Result<(f64, bool), SimError> {
    if t_age_gyr < 0.0 {
        return Err(SimError::Solver(format!(
            "solar age {t_age_gyr:.3} Gyr < 0 (before ZAMS); Gough undefined"
        )));
    }
    let t_fut = t_age_gyr - TAU_SUN_GYR;
    if t_fut <= 1.0 {
        Ok((gough_luminosity_ratio(t_fut), false))
    } else if t_fut <= 2.0 {
        Ok((gough_luminosity_ratio(t_fut), true)) // SOLAR_EXTRAPOLATION_WARNING
    } else {
        Err(SimError::Solver(format!(
            "t_fut {t_fut:.2} Gyr > 2: Gough forbidden standalone, use bahcall2001-blend"
        )))
    }
}

/// EUV/XUV flux relative to modern. Saturates at 100× below 0.1 Gyr.
pub fn euv_ratio(t_age_gyr: f64) -> f64 {
    if t_age_gyr < 0.1 {
        100.0
    } else {
        (t_age_gyr / TAU_SUN_GYR).powf(-1.2)
    }
}

/// Irradiance at 1 AU for a luminosity ratio (circular-orbit M1 baseline).
pub fn tsi_w_m2(l_ratio: f64) -> f64 {
    TSI0_W_M2 * l_ratio
}

// ---- Orbit lite (Berger 1978 / Laskar+2004 frequencies) ----
// Each component is an isolated pure sinusoid so its period is testable
// to ±1% by zero-crossing measurement (M1 gate).

pub fn ecc_400kyr(t_kyr: f64) -> f64 {
    0.010 * (2.0 * std::f64::consts::PI * t_kyr / 400.0).sin()
}
pub fn ecc_125kyr(t_kyr: f64) -> f64 {
    0.008 * (2.0 * std::f64::consts::PI * t_kyr / 125.0 + 1.1).sin()
}
pub fn ecc_95kyr(t_kyr: f64) -> f64 {
    0.006 * (2.0 * std::f64::consts::PI * t_kyr / 95.0 + 2.3).sin()
}
/// Mean eccentricity 0.028; range ≈ [0.004, 0.052].
pub fn eccentricity(t_kyr: f64) -> f64 {
    (0.028 + ecc_400kyr(t_kyr) + ecc_125kyr(t_kyr) + ecc_95kyr(t_kyr)).clamp(0.0, 0.08)
}
pub fn obliq_41kyr(t_kyr: f64) -> f64 {
    (2.0 * std::f64::consts::PI * t_kyr / 41.0).sin()
}
/// Obliquity in degrees, clamped to [22.1, 24.5] (spec §6.1).
pub fn obliquity_deg(t_kyr: f64) -> f64 {
    (23.3 + 1.2 * obliq_41kyr(t_kyr)).clamp(22.1, 24.5)
}
pub fn prec_23kyr(t_kyr: f64) -> f64 {
    (2.0 * std::f64::consts::PI * t_kyr / 23.0 + 0.7).sin()
}
pub fn prec_19kyr(t_kyr: f64) -> f64 {
    (2.0 * std::f64::consts::PI * t_kyr / 19.0 + 1.9).sin()
}
/// Longitude of perihelion (radians).
pub fn precession_angle(t_kyr: f64) -> f64 {
    let a = 2.0 * std::f64::consts::PI * t_kyr / 23.0 + 0.7 + 0.5 * prec_19kyr(t_kyr);
    a.rem_euclid(2.0 * std::f64::consts::PI)
}

// ---- Lunar / tides (LLR-anchored) ----

/// Lunar semi-major axis in km. Linear recession at the LLR-measured
/// 3.83 cm/yr (= 38.3 km/Myr) back from 384400 km. Documented M1
/// simplification: constant-Q; an evolving-Q model lands with M4 tides.
pub fn moon_distance_km(t_lb_ga: f64) -> f64 {
    384_400.0 - 38.3 * t_lb_ga * 1000.0
}

/// Length of day in hours through angular-momentum anchors:
/// 4.5 Ga → 6.5 h (spec range 5–8), 2.45 Ga → 17 h (16–18), present → 24 h.
pub fn day_length_hours(t_lb_ga: f64) -> f64 {
    if t_lb_ga <= 0.0 {
        24.0
    } else if t_lb_ga <= 2.45 {
        24.0 + (17.0 - 24.0) * (t_lb_ga / 2.45)
    } else {
        17.0 + (6.5 - 17.0) * ((t_lb_ga - 2.45) / (4.5 - 2.45)).min(1.0)
    }
}

/// Tidal mixing in TW, ∝ a^(−3), normalized to 1.5 TW present
/// (Munk–Wunsch 1–2 TW).
pub fn tidal_mixing_tw(moon_a_km: f64) -> f64 {
    1.5 * (384_400.0 / moon_a_km).powi(3)
}

pub struct SolarSystem;
impl Subsystem for SolarSystem {
    fn id(&self) -> &'static str { "solar" }
    fn dt_range(&self) -> (f64, f64) { (1.0, 1e6) }
    fn step(&mut self, world: &mut World, _dt: f64, _rng: &mut RngStream) -> SimResult<Diagnostics> {
        let t_age = solar_age_gyr(world.t_yr);
        let (l_ratio, extrapolated) = luminosity_ratio_at_age(t_age)?;
        if extrapolated {
            world.log("warning", &format!("SOLAR_EXTRAPOLATION_WARNING t_age={t_age:.2} Gyr"));
        }
        let t_lb_ga = (-world.t_yr / 1e9).max(0.0);
        world.totals.solar_l_ratio = l_ratio;
        world.totals.tsi_w_m2 = tsi_w_m2(l_ratio);
        world.totals.euv_ratio = euv_ratio(t_age);
        world.totals.day_hours = day_length_hours(t_lb_ga);
        world.totals.energy_w_m2 = 0.0;
        Ok(Diagnostics::default())
    }
    fn handshake(&self) -> Vec<FieldId> { vec![] }
}
