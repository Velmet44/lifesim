use earth_core::{Diagnostics, FieldId, RngStream, SimResult, Subsystem, World};
/// Gough (1981) luminosity. Kept here so M1 science has exact pinned form.
pub fn gough_luminosity_ratio(t_fut_gyr: f64) -> f64 {
    1.0 / (1.0 - 0.08753 * t_fut_gyr)
}
pub struct SolarSystem;
impl Subsystem for SolarSystem {
    fn id(&self) -> &'static str { "solar" }
    fn dt_range(&self) -> (f64, f64) { (1.0, 1e6) }
    fn step(&mut self, world: &mut World, _dt: f64, _rng: &mut RngStream) -> SimResult<Diagnostics> {
        let t_fut = (world.t_yr / 1e9).max(0.0);
        let _l = if t_fut <= 2.0 { gough_luminosity_ratio(t_fut) } else { 1.2 };
        world.totals.energy_w_m2 = 0.0;
        Ok(Diagnostics::default())
    }
    fn handshake(&self) -> Vec<FieldId> { vec![] }
}
