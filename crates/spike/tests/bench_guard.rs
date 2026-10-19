//! `spike-bench` connection rules (contracts/spike-bench.md; tasks T014).

use spike::bench::main_with;
use std::collections::HashMap;

const KEY: &str = "super-secret-api-key-0123456789";

fn run(args: &[&str], env: &[(&str, &str)]) -> (i32, String, String) {
    let env: HashMap<String, String> = env
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = main_with(&argv, &|k| env.get(k).cloned(), &mut out, &mut err);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

fn other(url: &str) -> Vec<(&str, &str)> {
    vec![("STASH_TEST_URL", url), ("STASH_TEST_API_KEY", KEY)]
}

#[test]
fn another_host_is_refused_without_allow_host() {
    let (code, _, err) = run(&["call-cost"], &other("http://stash.lan:9999"));
    assert_eq!(code, 3, "{err}");
    assert!(err.contains("not the test server"), "{err}");
}

#[test]
fn allow_host_lets_reads_reach_another_host() {
    // Port 1 refuses connections: past the guard, the run fails as unreachable (exit 4).
    let url = "http://127.0.0.1:1";
    let (code, _, err) = run(&["call-cost", "--allow-host", url], &other(url));
    assert_eq!(code, 4, "{err}");
    // --allow-host must name the same URL.
    let (code, _, _) = run(
        &["call-cost", "--allow-host", "http://elsewhere:1"],
        &other(url),
    );
    assert_eq!(code, 3);
}

#[test]
fn writes_are_refused_on_any_other_host_even_with_allow_host() {
    let url = "http://127.0.0.1:1";
    for sub in ["seed", "cleanup", "hooks"] {
        let (code, _, err) = run(&[sub, "--allow-host", url], &other(url));
        assert_eq!(code, 3, "{sub}: {err}");
        assert!(err.contains("writes to Stash"), "{sub}: {err}");
    }
}

#[test]
fn missing_environment_and_bad_arguments_are_usage_errors() {
    let (code, _, err) = run(&["call-cost"], &[("STASH_TEST_API_KEY", KEY)]);
    assert_eq!(code, 2, "{err}");
    let (code, _, err) = run(
        &["call-cost"],
        &[("STASH_TEST_URL", "http://localhost:9998")],
    );
    assert_eq!(code, 2, "{err}");
    assert_eq!(run(&[], &[]).0, 2);
    assert_eq!(run(&["frobnicate"], &[]).0, 2);
    assert_eq!(run(&["call-cost", "--scale", "huge"], &[]).0, 2);
    assert_eq!(run(&["--help"], &[]).0, 0);
}

#[test]
fn the_api_key_never_reaches_stdout_or_stderr() {
    let url = "http://127.0.0.1:1";
    for args in [
        vec!["call-cost", "--allow-host", url],
        vec!["seed"],
        vec!["call-cost"],
    ] {
        let (_, out, err) = run(&args, &other(url));
        assert!(
            !out.contains(KEY) && !err.contains(KEY),
            "{args:?}: {out}{err}"
        );
    }
}
