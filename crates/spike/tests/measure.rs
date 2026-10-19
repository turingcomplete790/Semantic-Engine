//! Measurement records (data-model "Measurement"; tasks T006).

use spike::measure::{
    write_result, Kind, Library, MeasureError, Measurement, Question, Series, Server,
};
use std::fs;
use std::path::PathBuf;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("spike-measure-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn base(id: &str, kind: Kind) -> Measurement {
    Measurement::new(
        id,
        Question::Process,
        kind,
        "spike-bench call-cost --scale test",
        Server {
            stash_version: Some("v0.31.1".into()),
            host: "http://localhost:9998".into(),
        },
        Library::test(1021, 1327, 6, 17, 0),
    )
}

fn latency(n: usize) -> Measurement {
    base("process-reopen-test", Kind::Latency).with_samples((1..=n).map(|v| v as f64).collect())
}

#[test]
fn statistics_are_recomputed_from_samples_on_load() {
    let dir = tmp("recompute");
    let path = write_result(&dir, &latency(100), &[]).unwrap();
    // Tamper with the stored summary: loading must ignore it.
    let mut v: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    v["median"] = 9999.0.into();
    v["p95"] = 1.0.into();
    v["runs"] = 3.into();
    fs::write(&path, v.to_string()).unwrap();

    let m = Measurement::load(&path).unwrap();
    assert_eq!(m.runs, 100);
    assert_eq!(m.median, 50.5);
    assert_eq!(m.p95, 95.0);
    assert_eq!(m.min, 1.0);
    assert_eq!(m.max, 100.0);
}

#[test]
fn latency_figures_need_at_least_100_runs() {
    assert!(matches!(
        latency(99).validate(),
        Err(MeasureError::TooFewRuns {
            runs: 99,
            needed: 100
        })
    ));
    assert!(latency(100).validate().is_ok());
    let dir = tmp("toofew");
    assert!(write_result(&dir, &latency(10), &[]).is_err());
    // Build and size figures need 3.
    let build = base("build-synth-test", Kind::Build).with_samples(vec![1.0, 2.0]);
    assert!(build.validate().is_err());
    assert!(build.with_samples(vec![1.0, 2.0, 3.0]).validate().is_ok());
}

#[test]
fn first_call_after_reload_is_kept_out_of_the_samples() {
    let mut s = Series::default();
    s.mark_reload();
    s.push(1300.0); // first call after the reload
    for _ in 0..100 {
        s.push(20.0);
    }
    s.mark_reload();
    s.push(1250.0);
    let (main, first) = s.into_measurements(base("process-reopen-test", Kind::Latency));
    assert_eq!(main.runs, 100);
    assert!(main.samples.iter().all(|v| *v == 20.0));
    let first = first.expect("a first-call measurement");
    assert_eq!(first.variant, "first-call");
    assert_eq!(first.id, "process-first-call");
    assert_eq!(first.samples, vec![1300.0, 1250.0]);
    assert!(
        first.validate().is_ok(),
        "first-call figures are observations, not 100-run series"
    );
}

#[test]
fn reruns_never_overwrite_an_existing_result() {
    let dir = tmp("rerun");
    let a = write_result(&dir, &latency(100), &[]).unwrap();
    let b = write_result(&dir, &latency(100), &[]).unwrap();
    let c = write_result(&dir, &latency(100), &[]).unwrap();
    assert_eq!(a.file_name().unwrap(), "process-reopen-test.json");
    assert_ne!(a, b);
    assert_ne!(b, c);
    for p in [&b, &c] {
        let name = p.file_name().unwrap().to_str().unwrap();
        assert!(
            name.starts_with("process-reopen-test-") && name.ends_with(".json"),
            "{name}"
        );
    }
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 3);
}

#[test]
fn secrets_never_reach_the_result_file() {
    let dir = tmp("secret");
    let key = "eyJhbGciOiJIUzI1NiJ9.secret-api-key";
    let cookie = "cookie-value-123";
    let mut m = latency(100);
    m.command = format!("spike-bench call-cost --api-key {key}");
    m.add_failure(3, "http", &format!("request with cookie {cookie} failed"));
    let path = write_result(&dir, &m, &[key, cookie]).unwrap();
    let text = fs::read_to_string(path).unwrap();
    assert!(!text.contains(key));
    assert!(!text.contains(cookie));
    assert!(text.contains("[redacted]"));
}

#[test]
fn json_fields_are_camel_case() {
    let v = serde_json::to_value(latency(100)).unwrap();
    for k in [
        "id",
        "question",
        "command",
        "server",
        "library",
        "runs",
        "samples",
        "median",
        "p95",
        "min",
        "max",
        "failures",
        "takenAt",
        "engineCommit",
    ] {
        assert!(v.get(k).is_some(), "missing {k}");
    }
    assert_eq!(v["question"], "process");
    assert_eq!(v["server"]["stashVersion"], "v0.31.1");
    assert_eq!(v["library"]["kind"], "test");
}

#[test]
fn stash_time_matches_stashs_local_text_form() {
    use spike::measure::{offset_of, stash_time};
    // 2026-10-17T23:57:53Z at -06:00 is the stored text Stash shows.
    let secs = 1_792_281_473;
    assert_eq!(stash_time(secs, -6 * 3600), "2026-10-17T17:57:53-06:00");
    assert_eq!(
        stash_time(secs, 5 * 3600 + 1800),
        "2026-10-18T05:27:53+05:30"
    );
    assert_eq!(offset_of("2026-10-17T17:57:53-06:00"), Some(-6 * 3600));
    assert_eq!(offset_of("2026-10-17T17:57:53+05:30"), Some(19800));
    assert_eq!(offset_of("2026-10-17T23:57:53Z"), Some(0));
    assert_eq!(offset_of("garbage"), None);
}
