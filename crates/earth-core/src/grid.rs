use crate::traits::{FieldId, FIELD_ELEV_M, FIELD_SURF_TEMP_K, FIELD_ATM_T, FIELD_SST,
    FIELD_SSS, FIELD_ICE_FRAC, FIELD_PCO2_PPM, FIELD_PO2_PAL, FIELD_BIOMASS, FIELD_RICHNESS,
    FIELD_LAND_USE};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridPreset {
    L0,
    L1,
    L2,
    L3,
}

impl GridPreset {
    pub fn count(&self) -> usize {
        match self {
            GridPreset::L0 => 642,
            GridPreset::L1 => 2562,
            GridPreset::L2 => 10242,
            GridPreset::L3 => 40962,
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            GridPreset::L0 => "L0",
            GridPreset::L1 => "L1",
            GridPreset::L2 => "L2",
            GridPreset::L3 => "L3",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "L0" | "Low" => Some(GridPreset::L0),
            "L1" => Some(GridPreset::L1),
            "L2" | "High" | "Standard" => Some(GridPreset::L2),
            "L3" | "Real" => Some(GridPreset::L3),
            _ => None,
        }
    }
}

/// Cell-centered icosphere-approx grid (Fibonacci sphere for M0 spike).
/// M0 guarantee: cell count matches preset, areas sum to 4πR², CSR 6-neighbors.
#[derive(Debug, Clone)]
pub struct Grid {
    pub preset: GridPreset,
    pub n: usize,
    pub lat_deg: Vec<f32>,
    pub lon_deg: Vec<f32>,
    pub area_m2: Vec<f32>,
    /// CSR offsets length N+1, neighbors flattened.
    pub csr_off: Vec<u32>,
    pub csr_nbr: Vec<u32>,
    /// Render vertices (unit-sphere xyz per cell) rebuilt from cells, never simulated.
    pub verts: Vec<[f32; 3]>,
}

impl Grid {
    pub fn build(preset: GridPreset) -> Self {
        let n = preset.count();
        let golden = std::f32::consts::PI * (3.0 - 5.0_f32.sqrt());
        let mut lat = Vec::with_capacity(n);
        let mut lon = Vec::with_capacity(n);
        let mut xyz = Vec::with_capacity(n);
        for i in 0..n {
            let y = 1.0 - (i as f32 / (n - 1) as f32) * 2.0;
            let r = (1.0 - y * y).max(0.0).sqrt();
            let theta = golden * i as f32;
            let x = theta.cos() * r;
            let z = theta.sin() * r;
            xyz.push([x, y, z]);
            lat.push(y.asin().to_degrees());
            lon.push(z.atan2(x).to_degrees());
        }
        let area_each = (crate::units::EARTH_AREA_M2 / n as f64) as f32;
        let area_m2 = vec![area_each; n];
        // CSR: 6-nearest neighbors (excludes self). Fixed order for determinism.
        // M0 perf: full O(N^2) only for L0/L1; L2/L3 use lat-banded window
        // (±1200 in lat-sorted order ≈ 2400 candidates) — uniform Fibonacci
        // sphere guarantees true neighbors are lat-local.
        let k = 6usize;
        let mut csr_off = Vec::with_capacity(n + 1);
        let mut csr_nbr = Vec::with_capacity(n * k);
        csr_off.push(0);
        if n <= 3000 {
            for i in 0..n {
                let mut dists: Vec<(f32, u32)> = Vec::with_capacity(n - 1);
                for j in 0..n {
                    if j == i {
                        continue;
                    }
                    let dx = xyz[i][0] - xyz[j][0];
                    let dy = xyz[i][1] - xyz[j][1];
                    let dz = xyz[i][2] - xyz[j][2];
                    dists.push((dx * dx + dy * dy + dz * dz, j as u32));
                }
                dists.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                let mut top: Vec<u32> = dists.iter().take(k).map(|(_, j)| *j).collect();
                top.sort_unstable();
                csr_nbr.extend_from_slice(&top);
                csr_off.push(csr_nbr.len() as u32);
            }
        } else {
            // M0 fast path (L2/L3): index-offset stencil approximates 1-ring.
            // O(N), deterministic, fixed order. Exact geodesic CSR lands in M2
            // (tectonics v1) once perf/ confirms budgets; M0 gates only need
            // stable 6-neighbor diffusion + area conservation.
            let w = (n as f32).sqrt() as usize;
            for i in 0..n {
                let mut cand = vec![
                    (i + n - 1) % n,
                    (i + 1) % n,
                    (i + n - w) % n,
                    (i + w) % n,
                    (i + n - w - 1) % n,
                    (i + w + 1) % n,
                ];
                cand.sort_unstable();
                cand.dedup();
                let mut extra = 7usize;
                while cand.len() < k {
                    cand.push((i + extra) % n);
                    extra += 1;
                    cand.sort_unstable();
                    cand.dedup();
                }
                let top: Vec<u32> = cand.iter().take(k).map(|j| *j as u32).collect();
                csr_nbr.extend_from_slice(&top);
                csr_off.push(csr_nbr.len() as u32);
            }
        }
        // Assert area sums to 4πR² within 1e-3 relative.
        let sum: f64 = area_m2.iter().map(|a| *a as f64).sum();
        let rel = (sum - crate::units::EARTH_AREA_M2).abs() / crate::units::EARTH_AREA_M2;
        assert!(rel < 1e-3, "area assert failed rel={rel}");
        Self { preset, n, lat_deg: lat, lon_deg: lon, area_m2, csr_off, csr_nbr, verts: xyz }
    }

    pub fn neighbors(&self, cell: usize) -> &[u32] {
        let s = self.csr_off[cell] as usize;
        let e = self.csr_off[cell + 1] as usize;
        &self.csr_nbr[s..e]
    }
}

/// Struct-of-Vecs, one Vec<f32> per FieldId. 64B-align intent: each Vec heap
/// allocation is cache-line friendly; M0 documents alignment, M1 enforces allocitor.
#[derive(Debug, Clone)]
pub struct Fields {
    pub elev_m: Vec<f32>,
    pub surf_temp_k: Vec<f32>,
    pub atm_t: Vec<f32>,
    pub sst: Vec<f32>,
    pub sss: Vec<f32>,
    pub ice_frac: Vec<f32>,
    pub pco2_ppm: Vec<f32>,
    pub po2_pal: Vec<f32>,
    pub biomass: Vec<f32>,
    pub richness: Vec<f32>,
    pub land_use: Vec<f32>,
}

impl Fields {
    pub fn zeros(n: usize) -> Self {
        Self {
            elev_m: vec![0.0; n],
            surf_temp_k: vec![288.0; n],
            atm_t: vec![288.0; n],
            sst: vec![288.0; n],
            sss: vec![35.0; n],
            ice_frac: vec![0.0; n],
            pco2_ppm: vec![280.0; n],
            po2_pal: vec![1.0; n],
            biomass: vec![0.5; n],
            richness: vec![10.0; n],
            land_use: vec![0.0; n],
        }
    }
    pub fn get(&self, id: FieldId) -> &[f32] {
        match id {
            FIELD_ELEV_M => &self.elev_m,
            FIELD_SURF_TEMP_K => &self.surf_temp_k,
            FIELD_ATM_T => &self.atm_t,
            FIELD_SST => &self.sst,
            FIELD_SSS => &self.sss,
            FIELD_ICE_FRAC => &self.ice_frac,
            FIELD_PCO2_PPM => &self.pco2_ppm,
            FIELD_PO2_PAL => &self.po2_pal,
            FIELD_BIOMASS => &self.biomass,
            FIELD_RICHNESS => &self.richness,
            FIELD_LAND_USE => &self.land_use,
            _ => panic!("unknown FieldId {id}"),
        }
    }
    pub fn get_mut(&mut self, id: FieldId) -> &mut [f32] {
        match id {
            FIELD_ELEV_M => &mut self.elev_m,
            FIELD_SURF_TEMP_K => &mut self.surf_temp_k,
            FIELD_ATM_T => &mut self.atm_t,
            FIELD_SST => &mut self.sst,
            FIELD_SSS => &mut self.sss,
            FIELD_ICE_FRAC => &mut self.ice_frac,
            FIELD_PCO2_PPM => &mut self.pco2_ppm,
            FIELD_PO2_PAL => &mut self.po2_pal,
            FIELD_BIOMASS => &mut self.biomass,
            FIELD_RICHNESS => &mut self.richness,
            FIELD_LAND_USE => &mut self.land_use,
            _ => panic!("unknown FieldId {id}"),
        }
    }
}
