# LifeSim — Earth Simulator (spec-driven, pre-implementation)

Research-cum-prediction earth simulator: cold-start from formation + digital-twin present + deep future.
Stack: Rust + GPUI + wgpu. Default grid L2 = 10,242 icosphere cells. Hybrid CPU/GPU kernels.

See spec.md (v1.2 integrated gpui+validation+compute+arch; science review pending; iterating until ≥08:27 UTC).
Public repo: https://github.com/Velmet44/lifesim

## Status
- [x] spec v1.0 + v1.1 (checkpoint tolerances, kernel table, config/trait sketches)
- [x] background reviews gpui/validation/compute/arch integrated (v1.2)
- [ ] science review pending → v1.3
- [x] scaffold: Cargo workspace, budgets, manifest, examples, field-registry, era-dt, ethics, CI stubs
- [ ] M0 scaffold (earth-core grid/time/kernels + GPUI×wgpu spike)

## Quick links
- `spec.md` — full specification
- `Cargo.toml` — workspace scaffold (crates not yet created, M0)
- `.review-*.md` — reviewer outputs (gitignored, integration notes go into spec Improvement Log)
