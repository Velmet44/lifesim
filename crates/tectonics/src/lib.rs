use earth_core::{Diagnostics, FieldId, RngStream, SimResult, Subsystem, World};
pub struct TectonicsSystem;
impl Subsystem for TectonicsSystem {
    fn id(&self) -> &'static str { "tectonics" }
    fn dt_range(&self) -> (f64, f64) { (1.0, 1e6) }
    fn step(&mut self, world: &mut World, dt_yr: f64, rng: &mut RngStream) -> SimResult<Diagnostics> {
        let amp = (dt_yr / 1e6).clamp(0.0, 1.0) as f32 * 2.0;
        for e in world.fields.elev_m.iter_mut() {
            let j = (rng.next_f32() - 0.5) * amp;
            *e = (*e + j).clamp(-11000.0, 9000.0);
        }
        Ok(Diagnostics::default())
    }
    fn handshake(&self) -> Vec<FieldId> { vec![1] }
}
