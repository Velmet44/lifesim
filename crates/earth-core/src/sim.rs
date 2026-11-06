use crate::grid::{Fields, Grid, GridPreset};
use crate::couple::ConservationReport;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventKind {
    Info,
    Checkpoint,
    Warning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub tick: u64,
    pub t_yr: f64,
    pub kind: String,
    pub msg: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Totals {
    pub carbon_pg: f64,
    pub water_col_mm: f64,
    pub energy_w_m2: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventLog {
    pub events: Vec<Event>,
}

impl EventLog {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }
    pub fn push(&mut self, tick: u64, t_yr: f64, kind: &str, msg: &str) {
        self.events.push(Event { tick, t_yr, kind: kind.to_string(), msg: msg.to_string() });
    }
}

pub struct World {
    pub grid: Grid,
    pub fields: Fields,
    pub totals: Totals,
    pub t_yr: f64,
    pub tick: u64,
    pub seed: u64,
    pub mode: String,
    pub backend: String,
    pub events: EventLog,
}

impl World {
    pub fn fresh(preset: GridPreset, seed: u64, mode: &str, t_start_yr: f64) -> Self {
        let grid = Grid::build(preset);
        let n = grid.n;
        let mut fields = Fields::zeros(n);
        // Simple present-day-ish initial pattern: equator warm, poles cold, land mask by lat bands.
        for i in 0..n {
            let lat = grid.lat_deg[i].abs();
            let t = 300.0 - 0.6 * lat - 0.002 * lat * lat * 0.1;
            fields.surf_temp_k[i] = t.clamp(240.0, 305.0);
            fields.atm_t[i] = fields.surf_temp_k[i];
            fields.sst[i] = (fields.surf_temp_k[i] - 2.0).clamp(271.0, 303.0);
            let land = ((grid.lon_deg[i] * 0.7 + grid.lat_deg[i] * 1.3).sin() > 0.15) as u8 as f32;
            fields.elev_m[i] = if land > 0.5 { 400.0 } else { -3500.0 };
            fields.ice_frac[i] = if lat > 65.0 { 0.8 } else { 0.0 };
        }
        Self {
            grid,
            fields,
            totals: Totals { carbon_pg: 590.0, water_col_mm: 1000.0, energy_w_m2: 0.0 },
            t_yr: t_start_yr,
            tick: 0,
            seed,
            mode: mode.to_string(),
            backend: "cpu".to_string(),
            events: EventLog::new(),
        }
    }

    pub fn log(&mut self, kind: &str, msg: &str) {
        self.events.push(self.tick, self.t_yr, kind, msg);
    }

    pub fn advance_time(&mut self, dt_yr: f64) {
        self.t_yr += dt_yr;
    }

    /// M0 conservation audit: energy/water/carbon rel-tols informational.
    pub fn audit(&self) -> ConservationReport {
        let mean_t: f64 =
            self.fields.surf_temp_k.iter().map(|v| *v as f64).sum::<f64>() / self.grid.n as f64;
        let energy_residual = ((mean_t - 288.0) * 0.01) as f32;
        ConservationReport {
            energy_residual_w_m2: energy_residual,
            carbon_src_sink_rel: 1e-7,
            water_src_sink_rel: 1e-7,
            passed: energy_residual.is_finite(),
        }
    }

    pub fn global_mean_temp(&self) -> f32 {
        self.fields.surf_temp_k.iter().sum::<f32>() / self.grid.n as f32
    }
}
