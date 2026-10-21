//! `report` over a results folder (tasks T041).

use spike::bench::report::{load, render};
use spike::measure::{write_result, Kind, Library, Measurement, Question, Server};

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("spike-report-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn run(id: &str, samples: Vec<f64>, taken: &str) -> Measurement {
    let mut m = Measurement::new(
        id,
        Question::Process,
        Kind::Build,
        "spike-bench test",
        Server::local(),
        Library::test(0, 0, 0, 0, 0),
    )
    .with_samples(samples);
    m.taken_at = taken.into();
    m
}

#[test]
fn newest_run_is_shown_and_a_rerun_over_25_percent_is_flagged() {
    let dir = tmp("drift");
    write_result(
        &dir,
        &run(
            "process-a",
            vec![10.0, 10.0, 10.0],
            "2026-01-01T00:00:00.000Z",
        ),
        &[],
    )
    .unwrap();
    write_result(
        &dir,
        &run(
            "process-a",
            vec![11.0, 11.0, 11.0],
            "2026-01-02T00:00:00.000Z",
        ),
        &[],
    )
    .unwrap();
    write_result(
        &dir,
        &run(
            "process-b",
            vec![10.0, 10.0, 10.0],
            "2026-01-01T00:00:00.000Z",
        ),
        &[],
    )
    .unwrap();
    write_result(
        &dir,
        &run(
            "process-b",
            vec![20.0, 20.0, 20.0],
            "2026-01-02T00:00:00.000Z",
        ),
        &[],
    )
    .unwrap();
    std::fs::write(
        dir.join("hooks.json"),
        r#"{"rows":[{},{}],"updatedAt":"x"}"#,
    )
    .unwrap();

    let (by_id, rejected) = load(&dir).unwrap();
    assert!(rejected.is_empty());
    assert_eq!(by_id["process-a"].newest().median, 11.0);
    assert!(!by_id["process-a"].flagged());
    assert!(by_id["process-b"].flagged());

    let text = render(&dir).unwrap();
    assert!(text.contains("## process"));
    assert!(text.contains("+10.0%"));
    assert!(text.contains("+100.0% **over 25%**"));
    assert!(text.contains("**Reruns over 25%:** process-b"));
    assert!(text.contains("2 hook observation rows"));
}

#[test]
fn a_file_with_too_few_runs_is_skipped_with_the_reason() {
    let dir = tmp("short");
    std::fs::create_dir_all(&dir).unwrap();
    let mut m = run("process-c", vec![1.0], "2026-01-01T00:00:00.000Z");
    m.recompute();
    std::fs::write(
        dir.join("process-c.json"),
        serde_json::to_string(&m).unwrap(),
    )
    .unwrap();
    let text = render(&dir).unwrap();
    assert!(text.contains("`process-c.json` was skipped"), "{text}");
}
