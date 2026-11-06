# Field Registry (stub — canonical table required before M0, see arch review C1/P2)
# Columns: FieldId u16 (STABLE, never renumber) | name | unit | dtype | valid_range | writer_subsystem | notes
# Writers must be unique per Coupler phase; Coupler::new panics on overlap.

| id | name | unit | dtype | range | writer | notes |
|----|------|------|-------|-------|--------|-------|
| 1 | elev_m | m | f32 | [-11000, 9000] | tectonics | + erosion updates within tectonics phase |
| 2 | crust_thick_km | km | f32 | [0, 80] | tectonics | — |
| 3 | crust_age_Myr | Myr | f32 | [0, 4000] | tectonics | reset at ridges |
| 4 | plate_id | — | u16 | [0, 32] | tectonics | — |
| 5 | mantle_heat_flux | mW/m2 | f32 | [0, 500] | interior | — |
| 6 | surf_temp_K | K | f32 | [150, 350] | atmos | ocean/ice read; audit checks |
| 10 | atm_T | K | f32 | [150, 330] | atmos | cell-centered |
| 11 | atm_q | kg/kg | f32 | [0, 0.05] | atmos | — |
| 20 | SST | K | f32 | [271, 310] | ocean | — |
| 21 | SSS | psu | f32 | [0, 42] | ocean | — |
| 30 | ice_frac | 1 | f32 | [0, 1] | ice | — |
| 40 | pCO2_ppm | ppm | f32 | [10, 100000] | geochem | log-scale for scoring |
| 41 | pO2_PAL | PAL | f32 | [1e-8, 3] | geochem | log-scale |
| 60 | biomass_producer | kgC/m2 | f32 | [0, 50] | eco | per functional group; extend |
| 61 | richness | count | u32 | [0, 1e9] | bio | — |
| 70 | land_use_frac | 1 | f32 | [0, 1] | civs | dormant unless thresholds |
| TODO | edge_u/v | m/s | f32 | — | atmos/ocean | EdgeSoA, counts per preset TODO |
