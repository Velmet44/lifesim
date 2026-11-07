//! M1 end-to-end: Hadean cold-start through F1/F2/F3 via the real CLI binary.
//! Runs `lifesim check --epoch <F> --json` and validates measured values
//! against spec §16 T1 targets. Single source of truth with the CLI runner.

use std::process::Command;

fn check_json(epoch: &str) -> serde_json::Value {
    let exe = env!("CARGO_BIN_EXE_lifesim");
    let out = Command::new(exe)
        .args(["--json", "check", "--epoch", epoch, "--offline"])
        .output()
        .expect("lifesim binary runs");
    assert!(out.status.success(), "check {epoch} failed: {}", String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stdout).expect("valid JSON")
}

#[test]
fn m1_f1_accretion_energy() {
    let v = check_json("F1");
    let r = &v["results"][0];
    assert_eq!(r["epoch"], "F1");
    assert!(r["pass"].as_bool().unwrap(), "F1 failed: {r}");
    let m = r["value"].as_f64().unwrap();
    assert!(m >= 1600.0, "F1 mean {m} K below Tm>1800K − 200K tol");
}

#[test]
fn m1_f2_solidification() {
    let v = check_json("F2");
    let r = &v["results"][0];
    assert_eq!(r["epoch"], "F2");
    assert!(r["pass"].as_bool().unwrap(), "F2 failed: {r}");
    let m = r["value"].as_f64().unwrap();
    assert!((400.0..=900.0).contains(&m), "F2 mean {m} K outside 400–900K");
    assert!(r["solidified_yr"].as_f64().is_some(), "no solidification record: {r}");
}

#[test]
fn m1_f3_oceans() {
    let v = check_json("F3");
    let r = &v["results"][0];
    assert_eq!(r["epoch"], "F3");
    assert!(r["pass"].as_bool().unwrap(), "F3 failed: {r}");
    let f = r["value"].as_f64().unwrap();
    assert!(f > 0.30, "F3 ocean frac {f} ≤ 30%");
}
