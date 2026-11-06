# M0 task breakdown (scaffold → globe spike + determinism)
# Goal: window renders globe texture (Path B), reruns bit-identical. Blocks all later milestones.

- [ ] T1 workspace crates: earth-units/grid/time/rng/traits/couple/snap + facade earth-core + earth-sim orchestrator (arch P1 DAG)
- [ ] T2 grid L0/L1/L2 presets + CSR + area assert + SoA 64B align (arch P3) + render verts
- [ ] T3 time manager phase DAG stub + watchdog halve-retry-abort + event log (arch P5)
- [ ] T4 kernels: CPU diffuse/advect + deterministic tiled reductions (arch P4) + DevicePolicy Auto→Cpu + staging_test
- [ ] T5 snapshot .lfs single-file postcard+zstd+blake3 + determinism-smoke CI (arch P7, §20)
- [ ] T6 earth-gpui Path B: offscreen wgpu → RGBA8 → gpui Image, threads sim/render/UI + proto enums (§21), keymap, watchdog dot
- [ ] T7 earth-cli init/run/check/export --offline on fallbacks + lifesim.toml v1 + manifest verification
- [ ] T8 perf benches kernels/coupler/e2e + budgets.toml tracking
- [ ] T9 acceptance: Path-B ≥2fps L0 CPU-only, single wgpu version, byte-equal rerun, UI <100ms, timeline <100ms/<200ms, export parity, keyboard headless playback

Exit gate: all T1–T9 green + golden screenshot hash stored.
