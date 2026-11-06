use earth_core::{Diagnostics, FieldId, RngStream, SimResult, Subsystem, World};
pub struct OceanSystem;
impl Subsystem for OceanSystem {
    fn id(&self) -> &'static str { "ocean" }
    fn dt_range(&self) -> (f64, f64) { (1.0 / 365.0, 10.0) }
    fn step(&mut self, world: &mut World, dt_yr: f64, _rng: &mut RngStream) -> SimResult<Diagnostics> {
        let a = (dt_yr.min(1.0)) as f32 * 0.1;
        for i in 0..world.grid.n {
            let sst = world.fields.sst[i];
            let target = (world.fields.surf_temp_k[i] - 1.5).clamp(271.0, 303.0);
            world.fields.sst[i] = sst + a * (target - sst);
        }
        Ok(Diagnostics::default())
    }
    fn handshake(&self) -> Vec<FieldId> { vec![20, 21] }
}
