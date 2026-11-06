use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use earth_core::{GridPreset, World};
use earth_kernels::{diffuse_cpu, tiled_sum};

fn bench_diffuse(c: &mut Criterion) {
    for (name, preset) in [("L0", GridPreset::L0), ("L1", GridPreset::L1), ("L2", GridPreset::L2)] {
        let world = World::fresh(preset, 42, "formation", -4_540_000_000.0);
        let mut field = world.fields.surf_temp_k.to_vec();
        c.bench_function(&format!("diffuse_{name}"), |b| {
            b.iter(|| diffuse_cpu(black_box(&world.grid), black_box(&mut field), black_box(0.1)))
        });
    }
}

fn bench_tiled_sum(c: &mut Criterion) {
    let world = World::fresh(GridPreset::L2, 42, "formation", -4_540_000_000.0);
    let field = world.fields.surf_temp_k.to_vec();
    c.bench_function("tiled_sum_L2", |b| b.iter(|| tiled_sum(black_box(&field))));
}

criterion_group!(benches, bench_diffuse, bench_tiled_sum);
criterion_main!(benches);
