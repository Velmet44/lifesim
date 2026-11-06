# LifeSim — Earth Simulator (spec-driven, pre-implementation)

Research-cum-prediction earth simulator: cold-start from formation + digital-twin present + deep future.
Stack: Rust + GPUI + wgpu. Default grid L2 = 10,242 icosphere cells. Hybrid CPU/GPU kernels.

See `spec.md` (currently v1.1+, iterating until ≥08:27 UTC with background reviewers).
Public repo: https://github.com/Velmet44/lifesim

## Status
- [x] spec v1.0 + v1.1 (checkpoint tolerances, kernel table, config/trait sketches)
- [ ] background reviews (science/arch/compute/validation/gpui) → integrate into v1.2+
- [ ] M0 scaffold (earth-core grid/time/kernels + GPUI×wgpu spike)

## Quick links
- `spec.md` — full specification
- `Cargo.toml` — workspace scaffold (crates not yet created, M0)
- `.review-*.md` — reviewer outputs (gitignored, integration notes go into spec Improvement Log)
