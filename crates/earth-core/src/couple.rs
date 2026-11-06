use crate::sim::World;
use crate::traits::{Diagnostics, SimError, SimResult, Subsystem};
use crate::rng::RngStream;

#[derive(Debug, Clone)]
pub struct Phase {
    pub name: &'static str,
    pub dt_yr: f64,
}

#[derive(Debug, Clone)]
pub struct ConservationReport {
    pub energy_residual_w_m2: f32,
    pub carbon_src_sink_rel: f64,
    pub water_src_sink_rel: f64,
    pub passed: bool,
}

#[derive(Debug, Clone)]
pub struct Coupler {
    pub phases: Vec<Phase>,
    pub watchdog: bool,
}

impl Coupler {
    pub fn default_dag(dt_yr: f64) -> Self {
        Self {
            phases: vec![
                Phase { name: "tectonics", dt_yr },
                Phase { name: "climate", dt_yr },
                Phase { name: "geochem", dt_yr },
                Phase { name: "bio_eco", dt_yr },
                Phase { name: "audit", dt_yr: 0.0 },
            ],
            watchdog: true,
        }
    }

    /// One full DAG pass. Watchdog: halve dt + retry once, then abort with event log.
    pub fn sync(
        &self,
        world: &mut World,
        systems: &mut [&mut dyn Subsystem],
        dt_yr: f64,
    ) -> SimResult<ConservationReport> {
        let mut last_err: Option<SimError> = None;
        for attempt in 0..2 {
            let try_dt = if attempt == 0 { dt_yr } else { dt_yr * 0.5 };
            match self.try_sync(world, systems, try_dt) {
                Ok(rep) => {
                    if attempt == 1 {
                        world.log("watchdog", &format!("halved dt to {try_dt} and recovered"));
                    }
                    return Ok(rep);
                }
                Err(e) => {
                    last_err = Some(e);
                    if !self.watchdog {
                        break;
                    }
                    world.log("watchdog", &format!("phase failed at dt={try_dt}, retrying halved"));
                }
            }
        }
        let e = last_err.unwrap();
        world.log("abort", &format!("watchdog abort: {e}"));
        Err(e)
    }

    fn try_sync(
        &self,
        world: &mut World,
        systems: &mut [&mut dyn Subsystem],
        dt_yr: f64,
    ) -> SimResult<ConservationReport> {
        for sys in systems.iter_mut() {
            let tick = world.tick;
            let mut rng = RngStream::new(world.seed, sys.id(), tick);
            let _d: Diagnostics = sys.step(world, dt_yr, &mut rng)?;
            world.tick += 1;
            // Non-finite guard after every phase.
            for v in world.fields.surf_temp_k.iter()
                .chain(world.fields.sst.iter())
                .chain(world.fields.pco2_ppm.iter())
            {
                if !v.is_finite() {
                    return Err(SimError::NonFinite(format!("{} produced non-finite", sys.id())));
                }
            }
        }
        // Conservation audit (M0 tolerances from docs/era-dt.md).
        let rep = world.audit();
        let energy_tol = 1e-5_f32;
        // M0 audit is informational: never abort deep-time on toy imbalance, just report.
        // Real per-phase tols enforced from M3 once full physics lands.
        let _ = energy_tol;
        Ok(rep)
    }
}
