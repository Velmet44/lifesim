use earth_core::{Diagnostics, FieldId, RngStream, SimResult, Subsystem, World};
pub struct GeochemSystem {
    pub weathering_k: f32,
    pub outgas_scale: f32,
}
impl Default for GeochemSystem {
    fn default() -> Self { Self { weathering_k: 1.0, outgas_scale: 1.0 } }
}
impl GeochemSystem {
    /// M1 era-aware CO₂ target: volatile inventory pre-4.0 Ga (formation
    /// outgassing; full carbonate-silicate lands in M4), preindustrial after.
    pub fn co2_target_ppm(t_yr: f64) -> f32 {
        if t_yr < -4_000_000_000.0 { 20000.0 } else { 280.0 }
    }
}
impl Subsystem for GeochemSystem {
    fn id(&self) -> &'static str { "geochem" }
    fn dt_range(&self) -> (f64, f64) { (1.0, 1e5) }
    fn step(&mut self, world: &mut World, dt_yr: f64, _rng: &mut RngStream) -> SimResult<Diagnostics> {
        let a = (dt_yr.min(1000.0) / 1000.0) as f32 * 0.02 * self.weathering_k;
        // Era-aware target (M1): formation inventory pre-4.0 Ga, else preindustrial.
        let target = Self::co2_target_ppm(world.t_yr) * self.outgas_scale;
        for i in 0..world.grid.n {
            let c = world.fields.pco2_ppm[i];
            world.fields.pco2_ppm[i] = (c + a * (target - c)).clamp(10.0, 100000.0);
        }
        Ok(Diagnostics::default())
    }
    fn handshake(&self) -> Vec<FieldId> { vec![40, 41] }
}
