# perf/ — budgets + benches (compute review §9)
# Budgets: see budgets.toml (dt-qualified; to be replaced by measured actuals on reference quad-core).
# Benches (criterion, to be implemented M0/M3):
# - kernels.rs: each kernel, L0/L1/L2/L3 x CPU/GPU (wall/step, GB/s effective, staging us/step)
# - coupler.rs: handshake cost per era regime
# - e2e.rs: sim-yr/wall-s per era (deep-time, Quaternary, twin)
# CI perf-check: L2 benches vs baseline ±15% warn / ±30% fail; nightly L3+GPU trend (non-blocking).
# Every output records backend, grid, seed, version hash.
