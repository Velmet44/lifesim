//! Formation: accretion → magma ocean → solidification → first crust (spec §6.2, M1).
//!
//! Windows (years, negative = past):
//! - Accretion + giant impacts:  [−4.56 Ga, −4.45 Ga] — drive toward 1800 K
//!   magma ocean; Theia scripted energy injection with seed jitter.
//! - Solidification:              [−4.45 Ga, −4.43 Ga] — relax toward 800 K;
//!   first global-mean < 900 K step records `totals.solidified_yr` + event log.
//! - Protocrust + outgassing:     (−4.43 Ga, −4.0 Ga] — deterministic protocrust
//!   relief, pCO₂ held at volatile inventory by geochem's era target.
//! After −4.0 Ga this system is a no-op (later milestones own the crust).

use earth_core::{Diagnostics, FieldId, RngStream, SimResult, Subsystem, World};

pub const ACCRETION_START_YR: f64 = -4_560_000_000.0;
pub const SOLIDUS_START_YR: f64 = -4_450_000_000.0;
pub const SOLIDUS_END_YR: f64 = -4_430_000_000.0;
pub const OUTGAS_END_YR: f64 = -4_000_000_000.0;
/// Theia impact anchor (scripted, seed-jittered): 4.51 Ga.
pub const THEIA_ANCHOR_YR: f64 = -4_510_000_000.0;
/// Magma-ocean target during accretion, K.
pub const MAGMA_K: f32 = 1800.0;
/// Solidification detection: first global mean below this (K).
pub const SOLIDIFY_MEAN_K: f32 = 900.0;
/// High-CO₂ volatile inventory target pre-4.0 Ga, ppm (spec §6.7 Archean band).
pub const VOLATILE_CO2_PPM: f32 = 20000.0;

pub struct FormationSystem {
    pub seed: u64,
    pub theia_jitter: f64,
    pub volatile_h2o: f64,
    pub theia_time_yr: f64,
    pub theia_energy: f64,
    solidified: bool,
}

impl FormationSystem {
    pub fn new(seed: u64, theia_jitter: f64, volatile_h2o: f64) -> Self {
        // Deterministic seed jitter: uniform u ∈ [0,1) from blake3(seed||"theia").
        let mut h = blake3::Hasher::new();
        h.update(&seed.to_le_bytes());
        h.update(b"||theia");
        let bytes = h.finalize();
        let u = u64::from_le_bytes(bytes.as_bytes()[0..8].try_into().unwrap()) as f64
            / u64::MAX as f64;
        let v = u64::from_le_bytes(bytes.as_bytes()[8..16].try_into().unwrap()) as f64
            / u64::MAX as f64;
        let theia_time_yr = THEIA_ANCHOR_YR + (u - 0.5) * 20_000_000.0 * theia_jitter;
        let theia_energy = 0.8 + 0.4 * v;
        Self { seed, theia_jitter, volatile_h2o: volatile_h2o.max(0.0), theia_time_yr, theia_energy, solidified: false }
    }

    pub fn is_accretion(t_yr: f64) -> bool {
        (ACCRETION_START_YR..SOLIDUS_START_YR).contains(&t_yr)
    }
    pub fn is_solidifying(t_yr: f64) -> bool {
        (SOLIDUS_START_YR..SOLIDUS_END_YR).contains(&t_yr)
    }
}

impl Default for FormationSystem {
    fn default() -> Self {
        Self::new(42, 0.1, 1.0)
    }
}

impl Subsystem for FormationSystem {
    fn id(&self) -> &'static str { "formation" }
    fn dt_range(&self) -> (f64, f64) { (1e3, 1e6) }
    fn step(&mut self, world: &mut World, dt_yr: f64, _rng: &mut RngStream) -> SimResult<Diagnostics> {
        let t = world.t_yr;
        if t < ACCRETION_START_YR || t > OUTGAS_END_YR {
            return Ok(Diagnostics::default()); // dormant outside Hadean formation
        }
        // Step-scaled relaxation (era dt up to 1 Myr): clamp inflates per-step
        // fraction so coarse deep-time steps still track stage targets.
        let k = ((dt_yr / 100_000.0).min(1.0).max(0.05)) as f32;
        if Self::is_accretion(t) {
            // Theia pulse: single-step spike when the handshake lands on it.
            let mut target = MAGMA_K;
            if (t - self.theia_time_yr).abs() <= dt_yr {
                target += (400.0 * self.theia_energy) as f32;
            }
            for v in world.fields.surf_temp_k.iter_mut() {
                *v = (*v + k * (target - *v)).clamp(200.0, 2400.0);
            }
            for v in world.fields.atm_t.iter_mut() {
                *v = (*v + k * (target - *v)).clamp(200.0, 2400.0);
            }
            // Molten surface: flat, ice-free, outgassed envelope.
            for v in world.fields.elev_m.iter_mut() {
                *v += k * (0.0 - *v);
            }
            for v in world.fields.ice_frac.iter_mut() {
                *v = 0.0;
            }
            let co2t = VOLATILE_CO2_PPM * self.volatile_h2o as f32;
            for v in world.fields.pco2_ppm.iter_mut() {
                *v = (*v + k * (co2t - *v)).clamp(10.0, 100000.0);
            }
            world.totals.outgassed_co2_ppm = co2t as f64;
        } else if Self::is_solidifying(t) {
            // Ramped freeze: target slides 1800 → 800 K across the window so
            // the <900 K detection lands near the window end (~4.43 Ga),
            // matching the spec solidification checkpoint.
            let frac = ((t - SOLIDUS_START_YR) / (SOLIDUS_END_YR - SOLIDUS_START_YR)) as f32;
            let target = 1800.0 + frac * (800.0 - 1800.0);
            for v in world.fields.surf_temp_k.iter_mut() {
                *v = (*v + k * (target - *v)).clamp(200.0, 2400.0);
            }
            for v in world.fields.atm_t.iter_mut() {
                *v = (*v + k * (target - *v)).clamp(200.0, 2400.0);
            }
            for v in world.fields.sst.iter_mut() {
                *v = (*v + k * (1200.0 - *v)).clamp(200.0, 2400.0);
            }
            let mean = world.global_mean_temp();
            if !self.solidified && mean < SOLIDIFY_MEAN_K {
                self.solidified = true;
                world.totals.solidified_yr = Some(t);
                world.log("checkpoint", &format!("magma-ocean solidified at {t:.0} yr (mean {mean:.0} K)"));
            }
        } else {
            // Protocrust: deterministic relief toward a fixed reference pattern;
            // outgassing inventory held until geochem takes over at −4.0 Ga.
            for i in 0..world.grid.n {
                let lon = world.grid.lon_deg[i].to_radians();
                let lat = world.grid.lat_deg[i].to_radians();
                let target = 1500.0 * (3.0 * lon).sin() * (2.0 * lat).cos() - 1000.0;
                let e = world.fields.elev_m[i];
                world.fields.elev_m[i] = (e + 0.02 * k * (target - e)).clamp(-11000.0, 9000.0);
            }
            let co2t = VOLATILE_CO2_PPM * self.volatile_h2o as f32;
            world.totals.outgassed_co2_ppm = co2t as f64;
        }
        Ok(Diagnostics::default())
    }
    fn handshake(&self) -> Vec<FieldId> { vec![6] }
}

/// F1 metric: global mean surface T at the checkpoint (target >1800 K, tol ±200 K).
pub fn evaluate_f1(world: &World) -> (f32, bool) {
    let m = world.global_mean_temp();
    (m, m >= 1600.0)
}

/// F2 metric: global mean surface T (target band 400–900 K).
pub fn evaluate_f2(world: &World) -> (f32, bool) {
    let m = world.global_mean_temp();
    (m, (400.0..=900.0).contains(&m))
}

/// F3 metric: liquid-water ocean fraction (target >30% of cells).
/// Ocean cell = below sea level AND SST in [273, 373] K.
pub fn evaluate_f3(world: &World) -> (f32, bool) {
    let mut ocean = 0usize;
    for i in 0..world.grid.n {
        let sst = world.fields.sst[i];
        if world.fields.elev_m[i] < 0.0 && (273.0..=373.0).contains(&sst) {
            ocean += 1;
        }
    }
    let frac = ocean as f32 / world.grid.n as f32;
    (frac, frac > 0.30)
}
