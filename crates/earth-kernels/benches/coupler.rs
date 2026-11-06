//! Coupler + e2e benches: full DAG pass cost per era regime + sim-yr/wall-s.
//! CI perf-check compares vs perf/budgets.toml ±15% warn / ±30% fail.
use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use earth_core::{Coupler, GridPreset, World};

fn full_systems(world: &mut World, dt_yr: f64) {
    let mut formation = formation::FormationSystem;
    let mut solar = solar::SolarSystem;
    let mut interior = interior::InteriorSystem;
    let mut tectonics = tectonics::TectonicsSystem;
    let mut atmos = atmos::AtmosSystem;
    let mut ocean = ocean::OceanSystem;
    let mut ice = ice::IceSystem;
    let mut geochem = geochem::GeochemSystem::default();
    let mut bio = bio::BioSystem;
    let mut eco = eco::EcoSystem;
    let mut civs = civs::CivsSystem::default();
    let coupler = Coupler::default_dag(dt_yr);
    let mut systems: Vec<&mut dyn earth_core::Subsystem> = vec![
        &mut formation, &mut solar, &mut interior, &mut tectonics, &mut atmos,
        &mut ocean, &mut ice, &mut geochem, &mut bio, &mut eco, &mut civs,
    ];
    coupler.sync(world, &mut systems, dt_yr).unwrap();
    world.advance_time(dt_yr);
}

fn bench_coupler(c: &mut Criterion) {
    // Deep-time handshake (dt=1kyr) vs twin daily handshake (dt=1yr).
    for (name, dt) in [("deep_time_1kyr", 1000.0), ("twin_1yr", 1.0)] {
        let mut world = World::fresh(GridPreset::L0, 42, "formation", -4_540_000_000.0);
        c.bench_function(&format!("coupler_L0_{name}"), |b| {
            b.iter(|| full_systems(black_box(&mut world), black_box(dt)))
        });
    }
}

fn bench_e2e(c: &mut Criterion) {
    // Sim-years per wall second at L0 deep-time: 10 DAG passes of 1kyr = 10kyr.
    c.bench_function("e2e_L0_deep_time_10kyr", |b| {
        b.iter(|| {
            let mut world = World::fresh(GridPreset::L0, 42, "formation", -4_540_000_000.0);
            for _ in 0..10 {
                full_systems(&mut world, 1000.0);
            }
            black_box(world.tick);
        })
    });
}

criterion_group!(benches, bench_coupler, bench_e2e);
criterion_main!(benches);
