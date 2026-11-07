use earth_core::{Diagnostics, FieldId, RngStream, SimResult, Subsystem, World};
pub struct AtmosSystem;
impl Subsystem for AtmosSystem {
    fn id(&self) -> &'static str { "atmos" }
    fn dt_range(&self) -> (f64, f64) { (1.0 / 365.0, 10.0) }
    fn step(&mut self, world: &mut World, dt_yr: f64, _rng: &mut RngStream) -> SimResult<Diagnostics> {
        // Pre-solidification there is no condensed atmosphere to model
        // (magma ocean + rock vapor live in formation); skip rather than clamp.
        if world.t_yr < -4_450_000_000.0 {
            return Ok(Diagnostics::default());
        }
        // EMBM-lite: relax toward latitudinal gradient scaled by insolation
        // ((S/S0)^1/4 blackbody factor; ≡1.0 for present-day runs) plus a
        // CO₂ greenhouse offset (M1 faint-young-sun closure, spec §23.1):
        // ΔT = 6.5·ln(C/280K), ≈ +28 K at the 20000 ppm Hadean inventory,
        // ≡0 at preindustrial. M3 replaces this with the band radiation scheme.
        let insol = (world.totals.tsi_w_m2 / 1361.0).powf(0.25).clamp(0.5, 2.0) as f32;
        let mut csum = 0.0f64;
        for v in world.fields.pco2_ppm.iter() {
            csum += *v as f64;
        }
        let cmean = (csum / world.grid.n as f64) as f32;
        let ghg = 6.5 * (cmean / 280.0).ln().max(0.0);
        let a = (dt_yr.min(1.0)) as f32 * 0.05;
        // Modern climate caps at 350 K; Hadean steam envelope runs hotter
        // (formation owns the magma range, atmos only sees ≤800 K inflow).
        let cap = if world.t_yr < -4_000_000_000.0 { 2400.0 } else { 350.0 };
        for i in 0..world.grid.n {
            let lat = world.grid.lat_deg[i].abs();
            let target = (300.0 - 0.55 * lat) * insol + ghg;
            let t = world.fields.surf_temp_k[i];
            let next = t + a * (target - t);
            world.fields.surf_temp_k[i] = next.clamp(150.0, cap);
            world.fields.atm_t[i] = world.fields.surf_temp_k[i];
        }
        Ok(Diagnostics::default())
    }
    fn handshake(&self) -> Vec<FieldId> { vec![6, 10] }
}
