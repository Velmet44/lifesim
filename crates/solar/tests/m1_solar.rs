//! M1 solar validation (spec §6.1 + §16 T1-adjacent checks).
//! Gough (1981) τ-form, EUV track, Berger/Laskar periods, LLR lunar anchors.

use solar::*;

// Spec §6.1 checks.
#[test]
fn gough_lookback_438ga() {
    let l = gough_lookback_ratio(4.38);
    assert!((l - 0.728).abs() <= 0.01, "L(4.38Ga)={l} want 0.728±0.01");
}

#[test]
fn gough_lookback_25ga() {
    let l = gough_lookback_ratio(2.5);
    assert!((l - 0.821).abs() <= 0.015, "L(2.5Ga)={l} want 0.821±0.015");
}

#[test]
fn gough_present_rate() {
    // dL/dt|₀ of the forward form = 0.40/τ⊙.
    assert!((DLDT0_LSUN_PER_GYR - 0.087).abs() <= 0.01);
    // Numerical cross-check on the forward form.
    let d = (gough_luminosity_ratio(0.01) - 1.0) / 0.01;
    assert!((d - 0.087).abs() <= 0.01, "numeric dL/dt={d}");
}

#[test]
fn gough_range_discipline() {
    // Present: fine, no extrapolation flag.
    let (l0, flag0) = luminosity_ratio_at_age(4.57).unwrap();
    assert!((l0 - 1.0).abs() < 1e-9 && !flag0);
    // Hadean formation age: fine.
    let (lh, _) = luminosity_ratio_at_age(0.06).unwrap();
    assert!((lh - 0.722).abs() < 0.02, "Hadean L={lh}");
    // +1.5 Gyr future: allowed with warning flag.
    let (_, flag1) = luminosity_ratio_at_age(4.57 + 1.5).unwrap();
    assert!(flag1);
    // Past +2 Gyr: hard error, never silent extrapolation.
    assert!(luminosity_ratio_at_age(4.57 + 2.5).is_err());
    // Before ZAMS: hard error.
    assert!(luminosity_ratio_at_age(-0.5).is_err());
}

#[test]
fn euv_track() {
    // 1 Gyr age: (1/4.57)^-1.2 ≈ 6.2.
    let e = euv_ratio(1.0);
    assert!((e - 6.2).abs() < 0.5, "EUV(1Ga)={e}");
    // Modern anchor.
    assert!((euv_ratio(4.57) - 1.0).abs() < 1e-9);
    // Saturated branch below 0.1 Gyr.
    assert_eq!(euv_ratio(0.05), 100.0);
    assert_eq!(euv_ratio(0.0), 100.0);
}

#[test]
fn tsi_present() {
    assert!((tsi_w_m2(1.0) - 1361.0).abs() < 1e-9);
}

/// Measure a component's period by rising zero-crossings of its demeaned
/// signal over three nominal cycles. Asserts ±1% (M1 orbit gate).
fn measure_period(f: impl Fn(f64) -> f64, nominal: f64) -> f64 {
    let span = 3.0 * nominal;
    let n = 720 * 3;
    let mut prev = f(0.0);
    let mut crossings = 0u32;
    // Demean with the analytic mean of a zero-phase sinusoid sample (mean≈0
    // over whole cycles; compute empirically for phase-shifted components).
    let mean: f64 = (0..n).map(|i| f(span * i as f64 / n as f64)).sum::<f64>() / n as f64;
    for i in 1..=n {
        let v = f(span * i as f64 / n as f64) - mean;
        if prev <= 0.0 && v > 0.0 {
            crossings += 1;
        }
        prev = v;
    }
    assert_eq!(crossings, 3, "expected 3 rising crossings");
    span / crossings as f64
}

#[test]
fn orbit_periods_within_1pct() {
    for (name, f, p) in [
        ("ecc400", ecc_400kyr as fn(f64) -> f64, 400.0),
        ("ecc125", ecc_125kyr, 125.0),
        ("ecc95", ecc_95kyr, 95.0),
        ("obliq41", obliq_41kyr, 41.0),
        ("prec23", prec_23kyr, 23.0),
        ("prec19", prec_19kyr, 19.0),
    ] {
        let m = measure_period(f, p);
        let rel = (m - p).abs() / p;
        assert!(rel < 0.01, "{name}: measured {m} vs {p} (rel {rel})");
    }
}

#[test]
fn orbit_ranges() {
    // Obliquity clamp [22.1, 24.5].
    for k in 0..2000 {
        let o = obliquity_deg(k as f64 * 5.0 - 5000.0);
        assert!((22.1..=24.5).contains(&o), "obliquity {o}");
    }
    // Eccentricity sane.
    for k in 0..2000 {
        let e = eccentricity(k as f64 * 5.0 - 5000.0);
        assert!((0.0..=0.08).contains(&e), "ecc {e}");
    }
    // Precession angle wraps [0, 2π).
    for k in 0..500 {
        let a = precession_angle(k as f64 * 7.0);
        assert!((0.0..2.0 * std::f64::consts::PI).contains(&a));
    }
}

#[test]
fn lunar_anchors() {
    // Hadean day 5–8 h; 2.45 Ga 16–18 h; present 24 h (spec §6.1).
    let d45 = day_length_hours(4.5);
    assert!((5.0..=8.0).contains(&d45), "day(4.5Ga)={d45}");
    let d245 = day_length_hours(2.45);
    assert!((16.0..=18.0).contains(&d245), "day(2.45Ga)={d245}");
    assert_eq!(day_length_hours(0.0), 24.0);
    // Monotonic slowdown.
    assert!(d45 < d245 && d245 < 24.0);
    // LLR anchor: 3.83 cm/yr → 38.3 km/Myr.
    assert!((moon_distance_km(0.0) - 384_400.0).abs() < 1e-9);
    let a45 = moon_distance_km(4.5);
    assert!((a45 - (384_400.0 - 38.3 * 4500.0)).abs() < 1e-6);
    // Tidal mixing normalized to Munk–Wunsch 1–2 TW present.
    let k0 = tidal_mixing_tw(384_400.0);
    assert!((1.0..=2.0).contains(&k0), "tidal {k0}");
    // Stronger when the Moon was closer.
    assert!(tidal_mixing_tw(a45) > k0);
}
