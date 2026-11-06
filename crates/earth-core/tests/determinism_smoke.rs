use earth_core::{Coupler, Subsystem, World, Diagnostics, FieldId, RngStream, SimResult, snapshot_hash};

struct Dummy;
impl Subsystem for Dummy {
    fn id(&self) -> &'static str { "dummy" }
    fn dt_range(&self) -> (f64, f64) { (1.0, 1e6) }
    fn step(&mut self, world: &mut World, _dt: f64, rng: &mut RngStream) -> SimResult<Diagnostics> {
        for v in world.fields.surf_temp_k.iter_mut() {
            *v = (*v + (rng.next_f32() - 0.5) * 0.01).clamp(150.0, 350.0);
        }
        Ok(Diagnostics::default())
    }
    fn handshake(&self) -> Vec<FieldId> { vec![6] }
}

fn run_once(seed: u64) -> String {
    let mut world = World::fresh(earth_core::GridPreset::L0, seed, "formation", -4_540_000_000.0);
    world.fields.assert_aligned();
    assert_eq!(world.grid.n, 642);
    // CSR validity: 6 neighbors each, no self-loops, in range.
    for i in 0..world.grid.n {
        let nb = world.grid.neighbors(i);
        assert_eq!(nb.len(), 6, "cell {i} must have 6 neighbors");
        for j in nb {
            assert!((*j as usize) < world.grid.n && (*j as usize) != i);
        }
    }
    let coupler = Coupler::default_dag(1000.0);
    let mut d = Dummy;
    let mut systems: Vec<&mut dyn Subsystem> = vec![&mut d];
    for _ in 0..100 {
        coupler.sync(&mut world, &mut systems, 1000.0).expect("sync");
        world.advance_time(1000.0);
    }
    snapshot_hash(&world)
}

#[test]
fn determinism_smoke() {
    // L0, 100 sync ticks, 2 reruns, blake3(snapshot) equal.
    let h1 = run_once(42);
    let h2 = run_once(42);
    assert_eq!(h1, h2, "reruns must be hash-equal");
}
