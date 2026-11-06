//! Physical constants and shared units.

pub const EARTH_RADIUS_M: f64 = 6_371_000.0;
pub const EARTH_AREA_M2: f64 = 4.0 * std::f64::consts::PI * EARTH_RADIUS_M * EARTH_RADIUS_M;
pub const TSI0_W_M2: f32 = 1361.0;
pub const L_SUN_W: f64 = 3.828e26;
pub const TAU_SUN_GYR: f64 = 4.57;
