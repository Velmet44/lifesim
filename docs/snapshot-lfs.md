# Snapshot .lfs format (from arch review P7)
# Single file: magic LFS1 u32 | header_len u32 | header postcard(...) | body postcard(Grid)+postcard(Totals)+EventLog tail | blake3(body) | zstd
# Header: {snap_v u32, engine_v String, grid preset, seed u64, mode u8, config_hash blake3, backend Cpu|Gpu, determinism Bitwise|Statistical, adapter String}
# Phylo tree: separate append-only .tree.lfs (parent u64, birth_yr f64, death_yr Option<f64>) — not per-tick, preserves ~400KB/tick budget.
# Reader rejects snap_v > supported with SimError::SnapshotVersion. Canonical TOML hashed (key-sorted), not raw file.
