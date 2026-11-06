use earth_core::{Grid, SimResult, Diagnostics, FieldId};
use rayon::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevicePolicy {
    Auto,
    CpuOnly,
    GpuPreferred,
    Hybrid,
}

#[derive(Debug, Clone)]
pub struct Scheduler {
    pub policy: DevicePolicy,
    pub staging_budget_mb: usize,
}

impl Scheduler {
    pub fn new(policy: DevicePolicy) -> Self {
        Self { policy, staging_budget_mb: 64 }
    }
    /// M0: always CPU. Probe adapters; software/no-GPU -> Cpu.
    pub fn backend(&self) -> &'static str {
        match self.policy {
            DevicePolicy::CpuOnly => "cpu",
            _ => "cpu",
        }
    }
    /// Startup staging test: measures host<->device copy; M0 CPU-only passes trivially.
    pub fn staging_test(&self) -> bool {
        true
    }
}

/// Deterministic tiled reduction (canonical algo): TILE=1024 indexed slots + serial final fold.
/// Never `par_iter().sum()` for science reductions.
pub fn tiled_sum(values: &[f32]) -> f64 {
    const TILE: usize = 1024;
    let mut slots = vec![0.0f64; (values.len() + TILE - 1) / TILE];
    slots
        .par_iter_mut()
        .enumerate()
        .for_each(|(t, acc)| {
            let s = t * TILE;
            let e = (s + TILE).min(values.len());
            let mut local = 0.0f64;
            for v in &values[s..e] {
                local += *v as f64;
            }
            *acc = local;
        });
    // Serial final fold in index order.
    slots.iter().sum()
}

/// CPU diffuse: 1-ring Laplacian smoothing toward neighbor mean.
pub fn diffuse_cpu(grid: &Grid, field: &mut [f32], kappa: f32) -> SimResult<Diagnostics> {
    let n = grid.n;
    assert_eq!(field.len(), n);
    let src = field.to_vec();
    let mut max_delta: f32 = 0.0;
    for i in 0..n {
        let nbrs = grid.neighbors(i);
        if nbrs.is_empty() {
            continue;
        }
        let mut mean = 0.0f32;
        for nb in nbrs {
            mean += src[*nb as usize];
        }
        mean /= nbrs.len() as f32;
        let next = src[i] + kappa * (mean - src[i]);
        max_delta = max_delta.max((next - src[i]).abs());
        field[i] = next;
    }
    Ok(Diagnostics { max_abs_delta: max_delta, energy_residual_w_m2: 0.0, steps_taken: 1 })
}

/// CPU advect stub: uniform zonal drift by `shift` cells (index rotation per latitude band).
/// Deterministic, conservative (permutation).
pub fn advect_stub(field: &mut [f32], _grid: &Grid, shift: usize) {
    if shift == 0 {
        return;
    }
    let n = field.len();
    let src = field.to_vec();
    let s = shift % n.max(1);
    for i in 0..n {
        field[i] = src[(i + n - s) % n];
    }
}

pub fn field_id_name(id: FieldId) -> &'static str {
    match id {
        1 => "elev_m",
        6 => "surf_temp_K",
        10 => "atm_T",
        20 => "SST",
        30 => "ice_frac",
        40 => "pCO2_ppm",
        _ => "unknown",
    }
}
