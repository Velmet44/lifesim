use earth_core::{GridPreset, World};
use earth_kernels::{diffuse_cpu, tiled_sum};
use std::time::Instant;

#[test]
fn perf_smoke() {
    // M0 perf harness stub (criterion benches land M3 with budgets.toml tracking).
    // Asserts deep-time-ish throughput is sane on L0; prints wall/step for CI trend.
    let world = World::fresh(GridPreset::L0, 42, "formation", -4_540_000_000.0);
    let mut field = world.fields.surf_temp_k.clone();
    let t0 = Instant::now();
    for _ in 0..10 {
        diffuse_cpu(&world.grid, &mut field, 0.1).unwrap();
    }
    let per_step = t0.elapsed().as_secs_f64() / 10.0;
    eprintln!("diffuse L0 per-step={per_step:.6}s");
    let s = tiled_sum(&field);
    assert!(s.is_finite());
    assert!(per_step < 0.5, "L0 diffuse too slow: {per_step}s/step");
}
