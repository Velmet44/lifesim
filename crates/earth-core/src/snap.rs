use crate::sim::World;
use crate::traits::{Backend, Determinism, SimError, SimResult};
use serde::{Deserialize, Serialize};

pub const SNAP_VERSION: u32 = 1;
pub const MAGIC: &[u8; 4] = b"LFS1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotHeader {
    pub snap_v: u32,
    pub engine_v: String,
    pub grid: String,
    pub seed: u64,
    pub mode: String,
    pub config_hash: String,
    pub backend: String,
    pub determinism: String,
    pub adapter: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SnapshotBody {
    tick: u64,
    t_yr: f64,
    n: usize,
    surf_temp_k: Vec<f32>,
    sst: Vec<f32>,
    ice_frac: Vec<f32>,
    pco2_ppm: Vec<f32>,
    elev_m: Vec<f32>,
    biomass: Vec<f32>,
}

pub fn snapshot_hash(world: &World) -> String {
    let mut h = blake3::Hasher::new();
    h.update(&world.tick.to_le_bytes());
    for v in world.fields.surf_temp_k.iter() {
        h.update(&v.to_le_bytes());
    }
    for v in world.fields.sst.iter() {
        h.update(&v.to_le_bytes());
    }
    h.finalize().to_hex().to_string()
}

pub fn save_snapshot(
    world: &World,
    header: &SnapshotHeader,
    path: &str,
) -> SimResult<String> {
    let header_bytes = postcard::to_stdvec(header).map_err(|e| SimError::Io(e.to_string()))?;
    let body = SnapshotBody {
        tick: world.tick,
        t_yr: world.t_yr,
        n: world.grid.n,
        surf_temp_k: world.fields.surf_temp_k.clone(),
        sst: world.fields.sst.clone(),
        ice_frac: world.fields.ice_frac.clone(),
        pco2_ppm: world.fields.pco2_ppm.clone(),
        elev_m: world.fields.elev_m.clone(),
        biomass: world.fields.biomass.clone(),
    };
    let body_bytes = postcard::to_stdvec(&body).map_err(|e| SimError::Io(e.to_string()))?;
    let compressed = zstd::encode_all(body_bytes.as_slice(), 3).map_err(|e| SimError::Io(e.to_string()))?;
    let hash = blake3::hash(&compressed);
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(header_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&header_bytes);
    out.extend_from_slice(&compressed);
    out.extend_from_slice(hash.as_bytes());
    std::fs::write(path, &out).map_err(|e| SimError::Io(e.to_string()))?;
    Ok(hash.to_hex().to_string())
}

pub fn load_snapshot(path: &str) -> SimResult<(SnapshotHeader, World, String)> {
    let data = std::fs::read(path).map_err(|e| SimError::Io(e.to_string()))?;
    if data.len() < 4 + 4 + 32 {
        return Err(SimError::Io("snapshot too short".into()));
    }
    if &data[0..4] != MAGIC {
        return Err(SimError::Io("bad magic, expected LFS1".into()));
    }
    let hlen = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
    if data.len() < 8 + hlen + 32 {
        return Err(SimError::Io("truncated snapshot".into()));
    }
    let header: SnapshotHeader =
        postcard::from_bytes(&data[8..8 + hlen]).map_err(|e| SimError::Io(e.to_string()))?;
    if header.snap_v > SNAP_VERSION {
        return Err(SimError::SnapshotVersion(header.snap_v));
    }
    let body_compressed = &data[8 + hlen..data.len() - 32];
    let stored_hash = &data[data.len() - 32..];
    let actual = blake3::hash(body_compressed);
    if actual.as_bytes() != stored_hash {
        return Err(SimError::Io("blake3 mismatch".into()));
    }
    let body_bytes =
        zstd::decode_all(body_compressed).map_err(|e| SimError::Io(e.to_string()))?;
    let body: SnapshotBody =
        postcard::from_bytes(&body_bytes).map_err(|e| SimError::Io(e.to_string()))?;
    let preset = crate::grid::GridPreset::parse(&header.grid).unwrap_or(crate::grid::GridPreset::L0);
    let mut world = World::fresh(preset, header.seed, &header.mode, body.t_yr);
    world.tick = body.tick;
    world.t_yr = body.t_yr;
    world.fields.surf_temp_k = body.surf_temp_k;
    world.fields.sst = body.sst;
    world.fields.ice_frac = body.ice_frac;
    world.fields.pco2_ppm = body.pco2_ppm;
    world.fields.elev_m = body.elev_m;
    world.fields.biomass = body.biomass;
    let _ = (Backend::Cpu, Determinism::Bitwise);
    Ok((header, world, actual.to_hex().to_string()))
}

pub fn make_header(world: &World, config_hash: &str) -> SnapshotHeader {
    SnapshotHeader {
        snap_v: SNAP_VERSION,
        engine_v: env!("CARGO_PKG_VERSION").to_string(),
        grid: world.grid.preset.name().to_string(),
        seed: world.seed,
        mode: world.mode.clone(),
        config_hash: config_hash.to_string(),
        backend: world.backend.clone(),
        determinism: "bitwise".to_string(),
        adapter: "cpu".to_string(),
    }
}
