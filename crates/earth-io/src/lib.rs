use earth_core::{SimError, SimResult, World};
use std::path::Path;

/// Verify data/manifest.toml exists; --offline uses embedded fallbacks.
pub fn verify_manifest(offline: bool) -> SimResult<()> {
    let candidates = ["data/manifest.toml", "../data/manifest.toml", "../../data/manifest.toml"];
    let found = candidates.iter().any(|p| Path::new(p).exists());
    if found {
        return Ok(());
    }
    if offline {
        // Embedded fallbacks pinned in manifest: allow missing file in offline mode.
        return Ok(());
    }
    Err(SimError::Io("data/manifest.toml missing; run with --offline for fallbacks".into()))
}

/// Export snapshot scope to CSV (M0). Parquet = CSV bytes with .parquet suffix + note.
pub fn export_csv(world: &World, path: &str) -> SimResult<()> {
    let mut s = String::from("cell,lat_deg,lon_deg,elev_m,surf_temp_K,sst,ice_frac,pco2_ppm,biomass\n");
    for i in 0..world.grid.n {
        s.push_str(&format!(
            "{},{:.4},{:.4},{:.2},{:.3},{:.3},{:.4},{:.2},{:.4}\n",
            i,
            world.grid.lat_deg[i],
            world.grid.lon_deg[i],
            world.fields.elev_m[i],
            world.fields.surf_temp_k[i],
            world.fields.sst[i],
            world.fields.ice_frac[i],
            world.fields.pco2_ppm[i],
            world.fields.biomass[i],
        ));
    }
    if path.ends_with(".parquet") {
        let note = "# M0 parquet-stub: CSV bytes with .parquet extension (real parquet in M7)\n";
        std::fs::write(path, note.to_string() + &s).map_err(|e| SimError::Io(e.to_string()))?;
    } else {
        std::fs::write(path, s).map_err(|e| SimError::Io(e.to_string()))?;
    }
    Ok(())
}

/// Scalar timeline row (one per DAG pass): tick,t_yr,mean_temp,mean_sst,mean_co2.
pub fn scalar_row(world: &World) -> String {
    let n = world.grid.n as f32;
    let mt: f32 = world.fields.surf_temp_k.iter().sum::<f32>() / n;
    let ms: f32 = world.fields.sst.iter().sum::<f32>() / n;
    let mc: f32 = world.fields.pco2_ppm.iter().sum::<f32>() / n;
    format!("{},{:.2},{:.4},{:.4},{:.3}", world.tick, world.t_yr, mt, ms, mc)
}
