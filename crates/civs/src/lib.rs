use earth_core::{Diagnostics, FieldId, RngStream, SimResult, Subsystem, World};
pub struct CivsSystem {
    pub enabled: bool,
}
impl Default for CivsSystem {
    fn default() -> Self { Self { enabled: true } }
}
impl Subsystem for CivsSystem {
    fn id(&self) -> &'static str { "civs" }
    fn dt_range(&self) -> (f64, f64) { (1.0, 100.0) }
    fn step(&mut self, world: &mut World, dt_yr: f64, _rng: &mut RngStream) -> SimResult<Diagnostics> {
        // Dormant until thresholds: only act in present/twin mode after 1900.
        let active = self.enabled && world.t_yr > 1900.0;
        if active {
            let g = (dt_yr.min(1.0)) as f32 * 0.001;
            for v in world.fields.land_use.iter_mut() {
                *v = (*v + g).clamp(0.0, 1.0);
            }
        }
        Ok(Diagnostics::default())
    }
    fn handshake(&self) -> Vec<FieldId> { vec![70] }
}
