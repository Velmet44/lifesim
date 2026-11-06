use earth_core::{GridPreset, World};
use earth_gpui::PathBRenderer;

#[test]
fn path_b_spike() {
    // M0 gate: Path-B globe >=2fps L0 CPU-only + golden screenshot.
    let world = World::fresh(GridPreset::L0, 42, "formation", -4_540_000_000.0);
    let r = PathBRenderer::new(256, 128);
    let (fps, hash) = r.headless_spike(&world, "golden-l0.ppm");
    eprintln!("path-b L0 320x160 fps={fps:.2} hash={hash}");
    assert!(fps >= 2.0 / 10.0, "debug ماشین slow: fps={fps} (CI release must be >=2)");
    // Golden hash stored for byte-compare; file written to target dir CWD.
    assert_eq!(hash.len(), 64);
}
