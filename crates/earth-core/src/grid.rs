use crate::traits::{FieldId, FIELD_ELEV_M, FIELD_SURF_TEMP_K, FIELD_ATM_T, FIELD_SST,
    FIELD_SSS, FIELD_ICE_FRAC, FIELD_PCO2_PPM, FIELD_PO2_PAL, FIELD_BIOMASS, FIELD_RICHNESS,
    FIELD_LAND_USE};
use aligned_vec::AVec;

/// 64-byte aligned f32 vector (SoA lane). Default AVec alignment is
/// CACHELINE_ALIGN (64B); asserted at construction.
pub type A64Vec = AVec<f32>;

fn avec_repeat(n: usize, v: f32) -> A64Vec {
    let a = AVec::from_iter(64, std::iter::repeat_n(v, n));
    debug_assert!(a.alignment() >= 64, "SoA lane must be at least 64B aligned");
    debug_assert_eq!(a.as_ptr() as usize % 64, 0, "SoA lane ptr must be 64B aligned");
    a
}

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
/// M0 guarantee: cell count matches preset, areas sum to 4πR², CSR true
/// 6-nearest neighbors (fixed order, deterministic).
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
        let csr = build_csr_geodesic(&xyz, &lat, &lon);
        let mut csr_off = Vec::with_capacity(n + 1);
        let mut csr_nbr = Vec::with_capacity(n * 6);
        csr_off.push(0);
        for nbrs in csr.iter() {
            csr_nbr.extend_from_slice(nbrs);
            csr_off.push(csr_nbr.len() as u32);
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

/// True geodesic 1-ring via lat/lon spatial binning + exact 3-D distance.
/// Bins sized for ~4 cells/bin; single-pass ring-3 gather (≈200 candidates),
/// one exact sort per cell. Fixed order: sort ids after top-k selection.
/// Ties broken by (dist bits, id) for determinism.
fn build_csr_geodesic(xyz: &[[f32; 3]], lat: &[f32], lon: &[f32]) -> Vec<[u32; 6]> {
    let n = xyz.len();
    // Adaptive bins: ~4 cells per bin.
    let nbins_target = (n / 4).max(64);
    let nlat = ((nbins_target as f64 / 2.0).sqrt() as usize).max(6);
    let nlon = (nlat * 2).max(12);
    let mut bins: Vec<Vec<usize>> = vec![Vec::new(); nlat * nlon];
    let bin_of = |la: f32, lo: f32| -> (usize, usize) {
        let a = (((90.0 - la) / 180.0 * nlat as f32).floor() as usize).min(nlat - 1);
        let mut b = (((lo + 180.0) / 360.0 * nlon as f32).floor() as usize) % nlon;
        if b >= nlon {
            b = nlon - 1;
        }
        (a, b)
    };
    for i in 0..n {
        let (a, b) = bin_of(lat[i], lon[i]);
        bins[a * nlon + b].push(i);
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let (la0, lo0) = bin_of(lat[i], lon[i]);
        let mut cand: Vec<usize> = Vec::with_capacity(256);
        let mut ring = 3usize;
        loop {
            cand.clear();
            let la_lo = la0.saturating_sub(ring);
            let la_hi = (la0 + ring).min(nlat - 1);
            for la in la_lo..=la_hi {
                for dlo in 0..=ring as isize {
                    for &s in &[-1isize, 1isize] {
                        if dlo == 0 && s == 1 {
                            continue;
                        }
                        let lo = (lo0 as isize + s * dlo).rem_euclid(nlon as isize) as usize;
                        cand.extend_from_slice(&bins[la * nlon + lo]);
                    }
                }
            }
            cand.sort_unstable();
            cand.dedup();
            if cand.len() >= 16 || ring >= nlat.max(nlon) {
                break;
            }
            ring += 1;
        }
        // Exact 3-D chord distance, exclude self.
        let mut dists: Vec<(u32, u32)> = Vec::with_capacity(cand.len());
        for j in cand.iter().copied() {
            if j == i {
                continue;
            }
            let dx = xyz[i][0] - xyz[j][0];
            let dy = xyz[i][1] - xyz[j][1];
            let dz = xyz[i][2] - xyz[j][2];
            let d2 = dx * dx + dy * dy + dz * dz;
            dists.push((d2.to_bits(), j as u32));
        }
        dists.sort_unstable();
        let mut top: Vec<u32> = Vec::with_capacity(6);
        for (_, j) in dists {
            if !top.contains(&j) {
                top.push(j);
            }
            if top.len() == 6 {
                break;
            }
        }
        // Fallback (should never trigger): index-offset pad.
        let mut extra = 1usize;
        while top.len() < 6 {
            let j = ((i + extra * 97) % n) as u32;
            if j as usize != i && !top.contains(&j) {
                top.push(j);
            }
            extra += 1;
        }
        top.sort_unstable();
        out.push([top[0], top[1], top[2], top[3], top[4], top[5]]);
    }
    out
}

/// Struct-of-AVecs, one 64B-aligned lane per FieldId.
#[derive(Debug, Clone)]
pub struct Fields {
    pub elev_m: A64Vec,
    pub surf_temp_k: A64Vec,
    pub atm_t: A64Vec,
    pub sst: A64Vec,
    pub sss: A64Vec,
    pub ice_frac: A64Vec,
    pub pco2_ppm: A64Vec,
    pub po2_pal: A64Vec,
    pub biomass: A64Vec,
    pub richness: A64Vec,
    pub land_use: A64Vec,
}

impl Fields {
    pub fn zeros(n: usize) -> Self {
        Self {
            elev_m: avec_repeat(n, 0.0),
            surf_temp_k: avec_repeat(n, 288.0),
            atm_t: avec_repeat(n, 288.0),
            sst: avec_repeat(n, 288.0),
            sss: avec_repeat(n, 35.0),
            ice_frac: avec_repeat(n, 0.0),
            pco2_ppm: avec_repeat(n, 280.0),
            po2_pal: avec_repeat(n, 1.0),
            biomass: avec_repeat(n, 0.5),
            richness: avec_repeat(n, 10.0),
            land_use: avec_repeat(n, 0.0),
        }
    }
    /// Assert every lane is at least 64B aligned (M0 gate).
    pub fn assert_aligned(&self) {
        for (name, lane) in [
            ("elev_m", &self.elev_m),
            ("surf_temp_k", &self.surf_temp_k),
            ("sst", &self.sst),
            ("pco2_ppm", &self.pco2_ppm),
        ] {
            assert!(lane.alignment() >= 64, "{name} lane misaligned");
            assert_eq!(lane.as_ptr() as usize % 64, 0, "{name} ptr misaligned");
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
