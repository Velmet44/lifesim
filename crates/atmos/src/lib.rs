use earth_core::{Diagnostics, FieldId, RngStream, SimResult, Subsystem, World};
pub struct AtmosSystem;
impl Subsystem for AtmosSystem {
    fn id(&self) -> &'static str { "atmos" }
    fn dt_range(&self) -> (f64, f64) { (1.0 / 365.0, 10.0) }
    fn step(&mut self, world: &mut World, dt_yr: f64, _rng: &mut RngStream) -> SimResult<Diagnostics> {
        // EMBM-lite stub: relax toward 288K + latitudinal gradient, then skip diffuse here
        // (diffuse applied by coupler-level kernel in M1; M0 keeps per-cell update deterministic).
        let a = (dt_yr.min(1.0)) as f32 * 0.05;
        for i in 0..world.grid.n {
            let lat = world.grid.lat_deg[i].abs();
            let target = 300.0 - 0.55 * lat;
            let t = world.fields.surf_temp_k[i];
            let next = t + a * (target - t);
            world.fields.surf_temp_k[i] = next.clamp(150.0, 350.0);
            world.fields.atm_t[i] = world.fields.surf_temp_k[i];
        }
        Ok(Diagnostics::default())
    }
    fn handshake(&self) -> Vec<FieldId> { vec![6, 10] }
}
