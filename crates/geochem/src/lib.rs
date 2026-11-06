use earth_core::{Diagnostics, FieldId, RngStream, SimResult, Subsystem, World};
pub struct GeochemSystem {
    pub weathering_k: f32,
    pub outgas_scale: f32,
}
impl Default for GeochemSystem {
    fn default() -> Self { Self { weathering_k: 1.0, outgas_scale: 1.0 } }
}
impl Subsystem for GeochemSystem {
    fn id(&self) -> &'static str { "geochem" }
    fn dt_range(&self) -> (f64, f64) { (1.0, 1e5) }
    fn step(&mut self, world: &mut World, dt_yr: f64, _rng: &mut RngStream) -> SimResult<Diagnostics> {
        let a = (dt_yr.min(1000.0) / 1000.0) as f32 * 0.02 * self.weathering_k;
        for i in 0..world.grid.n {
            let c = world.fields.pco2_ppm[i];
            // Relax toward 280 preindustrial; outgas adds tiny source.
            let target = 280.0 * self.outgas_scale;
            world.fields.pco2_ppm[i] = (c + a * (target - c)).clamp(10.0, 100000.0);
        }
        Ok(Diagnostics::default())
    }
    fn handshake(&self) -> Vec<FieldId> { vec![40, 41] }
}
