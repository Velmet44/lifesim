use earth_core::{Diagnostics, FieldId, RngStream, SimResult, Subsystem, World};
pub struct BioSystem;
impl Subsystem for BioSystem {
    fn id(&self) -> &'static str { "bio" }
    fn dt_range(&self) -> (f64, f64) { (1.0, 1e4) }
    fn step(&mut self, world: &mut World, dt_yr: f64, rng: &mut RngStream) -> SimResult<Diagnostics> {
        let g = (dt_yr.min(100.0) / 100.0) as f32 * 0.01;
        for i in 0..world.grid.n {
            let t = world.fields.surf_temp_k[i];
            let hab = if (273.0..340.0).contains(&t) { 1.0 } else { 0.0 };
            let r = world.fields.richness[i];
            world.fields.richness[i] = (r + g * hab * (r * 0.01 + rng.next_f32())).clamp(0.0, 1e9);
        }
        Ok(Diagnostics::default())
    }
    fn handshake(&self) -> Vec<FieldId> { vec![61] }
}
