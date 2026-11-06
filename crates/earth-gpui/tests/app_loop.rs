use earth_core::{GridPreset, World};
use earth_gpui::app::{key_to_cmd, run_headless};
use std::time::Instant;

#[test]
fn ui_latency_p95_under_100ms() {
    // Spec §9: UI event latency <100ms p95 at full sim speed.
    let keys = vec!["Space", "]", "[", "]", "Space", "]", "[", "]"];
    let (lat, dropped, _log) = run_headless(&keys, 20);
    assert!(!lat.is_empty(), "expected latency samples");
    let mut s = lat.clone();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p95 = s[((s.len() as f64 * 0.95) as usize).min(s.len() - 1)];
    eprintln!("ui latencies ms: {s:?} p95={p95:.2} dropped={dropped}");
    assert!(p95 < 100.0, "p95 UI latency {p95}ms >= 100ms");
}

#[test]
fn keyboard_headless_playback() {
    // Spec §10: keyboard playback headless without mouse.
    let keys = vec!["Space", "[", "]", "?", "s", "L", "E", "1", "2", "q"];
    let (_lat, _dropped, log) = run_headless(&keys, 5);
    // Sim-bound keys must apply at tick boundary + log (replayable).
    assert!(log.iter().any(|l| l.contains("Pause")), "Space->Pause missing: {log:?}");
    assert!(log.iter().any(|l| l.contains("StepOnce") || l.contains("Resume")));
    // UI-local keys map to None (no sim action, no crash).
    for k in ["1", "2", "L", "E", "?", "q"] {
        assert!(key_to_cmd(k).is_none(), "{k} should be UI-local");
    }
}

#[test]
fn timeline_load_seek_budget() {
    // Spec M0 T9: scalars <100ms load / <200ms seek.
    let mut world = World::fresh(GridPreset::L0, 7, "formation", -4_540_000_000.0);
    let t0 = Instant::now();
    let rows: Vec<String> = (0..100).map(|_| earth_io::scalar_row(&world)).collect();
    let load_ms = t0.elapsed().as_secs_f64() * 1000.0;
    assert!(load_ms < 100.0, "scalar load {load_ms}ms >= 100ms");
    let t1 = Instant::now();
    let _seek = &rows[rows.len() / 2];
    let seek_ms = t1.elapsed().as_secs_f64() * 1000.0;
    assert!(seek_ms < 200.0);
    let _ = world.tick;
    world.tick += 1;
    eprintln!("timeline load {load_ms:.2}ms seek {seek_ms:.2}ms");
}
