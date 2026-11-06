use earth_core::{Diagnostics, FieldId, RngStream, SimResult, Subsystem, World};
pub struct IceSystem;
impl Subsystem for IceSystem {
    fn id(&self) -> &'static str { "ice" }
    fn dt_range(&self) -> (f64, f64) { (1.0 / 365.0, 10.0) }
    fn step(&mut self, world: &mut World, _dt: f64, _rng: &mut RngStream) -> SimResult<Diagnostics> {
        for i in 0..world.grid.n {
            let t = world.fields.surf_temp_k[i];
            let target = if t < 271.0 { 0.9 } else if t < 275.0 { 0.4 } else { 0.0 };
            world.fields.ice_frac[i] += 0.2 * (target - world.fields.ice_frac[i]);
            world.fields.ice_frac[i] = world.fields.ice_frac[i].clamp(0.0, 1.0);
        }
        Ok(Diagnostics::default())
    }
    fn handshake(&self) -> Vec<FieldId> { vec![30] }
}
