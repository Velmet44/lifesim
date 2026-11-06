use earth_core::{Diagnostics, FieldId, RngStream, SimResult, Subsystem, World};
pub struct EcoSystem;
impl Subsystem for EcoSystem {
    fn id(&self) -> &'static str { "eco" }
    fn dt_range(&self) -> (f64, f64) { (1.0, 1e3) }
    fn step(&mut self, world: &mut World, dt_yr: f64, _rng: &mut RngStream) -> SimResult<Diagnostics> {
        let g = (dt_yr.min(10.0) / 10.0) as f32 * 0.02;
        for i in 0..world.grid.n {
            let t = world.fields.surf_temp_k[i];
            // Sharpe-Schoolfield-lite: peak near 288K, collapse outside.
            let f_t = ((-(t - 288.0).powi(2) / 800.0).exp()).clamp(0.0, 1.0);
            let b = world.fields.biomass[i];
            let target = 2.0 * f_t;
            world.fields.biomass[i] = (b + g * (target - b)).clamp(0.0, 50.0);
        }
        Ok(Diagnostics::default())
    }
    fn handshake(&self) -> Vec<FieldId> { vec![60] }
}
