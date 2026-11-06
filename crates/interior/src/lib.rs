use earth_core::{Diagnostics, FieldId, RngStream, SimResult, Subsystem, World};
pub struct InteriorSystem;
impl Subsystem for InteriorSystem {
    fn id(&self) -> &'static str { "interior" }
    fn dt_range(&self) -> (f64, f64) { (1e3, 1e6) }
    fn step(&mut self, world: &mut World, dt_yr: f64, _rng: &mut RngStream) -> SimResult<Diagnostics> {
        // Parameterized convection stub: tiny secular elev drift, bounded.
        let k = (dt_yr / 1e6).clamp(0.0, 1.0) as f32 * 0.01;
        for e in world.fields.elev_m.iter_mut() {
            *e = (*e + k).clamp(-11000.0, 9000.0);
        }
        Ok(Diagnostics::default())
    }
    fn handshake(&self) -> Vec<FieldId> { vec![1] }
}
