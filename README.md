# LifeSim — Earth Simulator (spec-driven, pre-implementation)

Research-cum-prediction earth simulator: cold-start from formation + digital-twin present + deep future.
Stack: Rust + GPUI + wgpu. Default grid L2 = 10,242 icosphere cells. Hybrid CPU/GPU kernels.

See spec.md (v1.2 integrated gpui+validation+compute+arch; science review pending; iterating until ≥08:27 UTC).
Public repo: https://github.com/Velmet44/lifesim

## Status
- [x] spec v1.0 + v1.1 (checkpoint tolerances, kernel table, config/trait sketches)
- [x] all 5 reviews integrated (v1.3) + adversarial audit 20/20 (v1.4)
- [x] docs: twin-init, deep-future, field-registry, era-dt, ethics, snapshot, M0 tasks, CI
- [x] scaffold: Cargo workspace, budgets, manifest, examples, field-registry, era-dt, ethics, CI stubs
- [x] M0 scaffold (earth-core grid/time/kernels + GPUI×wgpu spike)
      - true geodesic CSR (binned, all presets), 64B SoA lanes, threads+latency gates green
      - `cargo test --workspace` + `cargo test -p earth-gpui --features gpui-ui` (ImageSource interop)
- [x] M1 solar+formation (Gough track, EUV, Berger/Laskar ±1%, LLR moon; accretion→Theia→solidification ~4.43Ga→oceans>30%)

## Quick links
- `spec.md` — full specification
- `Cargo.toml` — workspace scaffold (crates not yet created, M0)
- `.review-*.md` — reviewer outputs (gitignored, integration notes go into spec Improvement Log)
