use crate::sim::World;
use crate::rng::RngStream;

pub type FieldId = u16;

// Stable IDs (never renumber). Canonical registry: docs/field-registry.md.
pub const FIELD_ELEV_M: FieldId = 1;
pub const FIELD_CRUST_THICK_KM: FieldId = 2;
pub const FIELD_CRUST_AGE_MYR: FieldId = 3;
pub const FIELD_PLATE_ID: FieldId = 4;
pub const FIELD_MANTLE_HEAT_FLUX: FieldId = 5;
pub const FIELD_SURF_TEMP_K: FieldId = 6;
pub const FIELD_ATM_T: FieldId = 10;
pub const FIELD_ATM_Q: FieldId = 11;
pub const FIELD_SST: FieldId = 20;
pub const FIELD_SSS: FieldId = 21;
pub const FIELD_ICE_FRAC: FieldId = 30;
pub const FIELD_PCO2_PPM: FieldId = 40;
pub const FIELD_PO2_PAL: FieldId = 41;
pub const FIELD_BIOMASS: FieldId = 60;
pub const FIELD_RICHNESS: FieldId = 61;
pub const FIELD_LAND_USE: FieldId = 70;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Cpu,
    Gpu,
    CpuFallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Determinism {
    Bitwise,
    Statistical,
}

#[derive(Debug, Clone)]
pub struct Diagnostics {
    pub max_abs_delta: f32,
    pub energy_residual_w_m2: f32,
    pub steps_taken: u64,
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self { max_abs_delta: 0.0, energy_residual_w_m2: 0.0, steps_taken: 1 }
    }
}

#[derive(Debug, Clone)]
pub enum SimError {
    Conservation(String),
    NonFinite(String),
    SnapshotVersion(u32),
    Io(String),
    Solver(String),
}

impl std::fmt::Display for SimError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimError::Conservation(s) => write!(f, "Conservation: {s}"),
            SimError::NonFinite(s) => write!(f, "NonFinite: {s}"),
            SimError::SnapshotVersion(v) => write!(f, "SnapshotVersion: {v}"),
            SimError::Io(s) => write!(f, "Io: {s}"),
            SimError::Solver(s) => write!(f, "Solver: {s}"),
        }
    }
}
impl std::error::Error for SimError {}

pub type SimResult<T> = Result<T, SimError>;

/// Canonical subsystem trait (M0). Subsystems borrow World mutably in DAG order.
pub trait Subsystem: Send {
    fn id(&self) -> &'static str;
    fn dt_range(&self) -> (f64, f64);
    fn step(&mut self, world: &mut World, dt_yr: f64, rng: &mut RngStream) -> SimResult<Diagnostics>;
    fn handshake(&self) -> Vec<FieldId>;
}
