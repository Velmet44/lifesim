use earth_core::{Diagnostics, FieldId, RngStream, SimResult, Subsystem, World};
pub struct FormationSystem;
impl Subsystem for FormationSystem {
    fn id(&self) -> &'static str { "formation" }
    fn dt_range(&self) -> (f64, f64) { (1e3, 1e6) }
    fn step(&mut self, world: &mut World, _dt: f64, _rng: &mut RngStream) -> SimResult<Diagnostics> {
        if world.t_yr < -4_400_000_000.0 {
            for t in world.fields.surf_temp_k.iter_mut() {
                *t = (*t + 12.0).clamp(200.0, 2000.0);
            }
        }
        Ok(Diagnostics::default())
    }
    fn handshake(&self) -> Vec<FieldId> { vec![6] }
}
