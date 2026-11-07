//! M1 formation unit tests: Theia determinism, stage windows, metric fns.

use earth_core::{GridPreset, World};
use formation::*;

#[test]
fn theia_deterministic_per_seed() {
    let a = FormationSystem::new(42, 0.1, 1.0);
    let b = FormationSystem::new(42, 0.1, 1.0);
    assert_eq!(a.theia_time_yr, b.theia_time_yr);
    assert_eq!(a.theia_energy, b.theia_energy);
    // Near the 4.51 Ga anchor, within jitter envelope.
    assert!((a.theia_time_yr - THEIA_ANCHOR_YR).abs() <= 10_000_000.0 * 0.1 + 1.0);
    assert!((0.8..=1.2).contains(&a.theia_energy));
}

#[test]
fn theia_varies_with_seed() {
    let a = FormationSystem::new(1, 1.0, 1.0);
    let b = FormationSystem::new(2, 1.0, 1.0);
    assert_ne!(a.theia_time_yr, b.theia_time_yr);
}

#[test]
fn stage_windows() {
    assert!(FormationSystem::is_accretion(-4_500_000_001.0));
    assert!(!FormationSystem::is_accretion(-4_450_000_000.0));
    assert!(FormationSystem::is_solidifying(-4_440_000_000.0));
    assert!(!FormationSystem::is_solidifying(-4_430_000_000.0));
    assert_eq!(FormationSystem::default().seed, 42);
}

#[test]
fn metrics_on_synthetic_worlds() {
    // Hot magma world fails F2 (too hot), passes F1.
    let mut hot = World::fresh(GridPreset::L0, 1, "formation", -4_560_000_000.0);
    let (m1, p1) = evaluate_f1(&hot);
    assert!(m1 > 1600.0 && p1);
    let (_, p2) = evaluate_f2(&hot);
    assert!(!p2);
    // Temperate post-solidification world passes F2 + F3.
    for v in hot.fields.surf_temp_k.iter_mut() {
        *v = 600.0;
    }
    for v in hot.fields.sst.iter_mut() {
        *v = 285.0;
    }
    for v in hot.fields.elev_m.iter_mut() {
        *v = -3000.0;
    }
    let (m2, p2) = evaluate_f2(&hot);
    assert!(p2, "F2 {m2}");
    let (f3, p3) = evaluate_f3(&hot);
    assert!((f3 - 1.0).abs() < 1e-6 && p3);
    // Frozen world fails F3 (no liquid water).
    for v in hot.fields.sst.iter_mut() {
        *v = 250.0;
    }
    let (_, p3f) = evaluate_f3(&hot);
    assert!(!p3f);
}
