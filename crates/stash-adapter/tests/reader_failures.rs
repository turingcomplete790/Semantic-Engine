//! Distinct, credential-free failures; retries only for transient ones; the version check
//! (spec 005 T023; FR-016 to FR-019, SC-006, SC-007).

mod common;

use common::{library, library_handler, root_field, serve};
use serde_json::json;
use stash_adapter::reader::{Kind, LibraryReader, MediaKind};
use stash_adapter::{Client, Error};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

const SECRET: &str = "SECRET-KEY-005";

/// A client with timeouts generous enough for a loaded test machine.
fn client_on(port: u16) -> Client {
    client_with(port, Duration::from_secs(10))
}

fn client_with(port: u16, request: Duration) -> Client {
    Client::with_api_key(&format!("http://127.0.0.1:{port}"), SECRET)
        .unwrap()
        .with_timeouts(Duration::from_secs(2), request)
        .unwrap()
}

/// A server that answers the version check normally and every other request with `status`
/// and `body`.
fn failing(status: u16, body: &'static str) -> (u16, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let (port, _) = serve(move |b| {
        if root_field(b) == "version" {
            return (
                200,
                json!({"data": {"version": {"version": "v0.31.1"}}}).to_string(),
            );
        }
        seen.fetch_add(1, Ordering::SeqCst);
        (status, body.to_string())
    });
    (port, calls)
}

fn no_secret(e: &Error) {
    assert!(!e.to_string().contains(SECRET), "{e}");
    assert!(!format!("{e:?}").contains(SECRET), "{e:?}");
}

fn ids(c: &Client) -> Result<Vec<String>, Error> {
    LibraryReader::all_ids(c, Kind::Scene)
}

#[test]
fn a_closed_port_is_unreachable() {
    // Port 1 is never listened on. A port freed by another test could be handed straight to a
    // fake server running in parallel.
    let port = 1;
    let e = ids(&client_on(port)).unwrap_err();
    assert!(matches!(e, Error::Unreachable(_)), "{e:?}");
    assert!(e.is_transient());
    no_secret(&e);
}

#[test]
fn a_server_that_never_answers_times_out() {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let _held: Vec<_> = l.incoming().take(8).collect();
        std::thread::sleep(Duration::from_secs(30));
    });
    let e = ids(&client_with(port, Duration::from_millis(300))).unwrap_err();
    assert!(matches!(e, Error::TimedOut(_)), "{e:?}");
    assert!(e.is_transient());
    no_secret(&e);
}

#[test]
fn rejected_credentials_are_not_authorized_and_not_retried() {
    for status in [401, 403] {
        let (port, _) = serve(move |_| (status, String::new()));
        let e = ids(&client_on(port)).unwrap_err();
        assert!(matches!(e, Error::NotAuthorized), "{status}: {e:?}");
        assert!(!e.is_transient());
        no_secret(&e);
    }
    let (port, calls) = failing(401, "");
    let e = ids(&client_on(port)).unwrap_err();
    assert!(matches!(e, Error::NotAuthorized));
    assert_eq!(calls.load(Ordering::SeqCst), 1, "never retried");
}

#[test]
fn a_busy_server_is_retried_twice_then_unreachable() {
    let (port, calls) = failing(503, "");
    let e = ids(&client_on(port)).unwrap_err();
    assert!(matches!(e, Error::Unreachable(_)), "{e:?}");
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    no_secret(&e);
}

#[test]
fn a_busy_server_that_recovers_succeeds() {
    let lib = library("library.json");
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let inner = library_handler(lib);
    let (port, _) = serve(move |b| {
        if root_field(b) != "version" && seen.fetch_add(1, Ordering::SeqCst) == 0 {
            return (503, String::new());
        }
        inner(b)
    });
    assert_eq!(ids(&client_on(port)).unwrap(), ["20", "21", "22", "23"]);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn an_old_or_unknown_version_is_unsupported_and_checked_once() {
    let (port, _) = serve(library_handler(json!({"version": "v0.30.0"})));
    let c = client_on(port);
    match LibraryReader::all_ids(&c, Kind::Tag).unwrap_err() {
        Error::UnsupportedVersion { found, minimum } => {
            assert_eq!((found.as_str(), minimum.as_str()), ("v0.30.0", "v0.31.1"))
        }
        e => panic!("{e:?}"),
    }
    assert_eq!(c.stash_version().unwrap().as_deref(), Some("v0.30.0"));

    let (port, _) = serve(library_handler(json!({"version": "nonsense"})));
    assert!(matches!(
        LibraryReader::all_ids(&client_on(port), Kind::Tag).unwrap_err(),
        Error::UnsupportedVersion { .. }
    ));

    let versions = Arc::new(AtomicUsize::new(0));
    let seen = versions.clone();
    let inner = library_handler(library("library.json"));
    let (port, _) = serve(move |b| {
        if root_field(b) == "version" {
            seen.fetch_add(1, Ordering::SeqCst);
        }
        inner(b)
    });
    let c = client_on(port);
    LibraryReader::all_ids(&c, Kind::Tag).unwrap();
    LibraryReader::all_ids(&c, Kind::Scene).unwrap();
    c.full_read(&|| false, &mut |_| {}).unwrap();
    assert_eq!(
        versions.load(Ordering::SeqCst),
        1,
        "checked once per client"
    );
}

#[test]
fn errors_reported_by_stash_are_graphql_and_not_retried() {
    let (port, calls) = failing(200, r#"{"errors":[{"message":"no such field"}]}"#);
    let e = ids(&client_on(port)).unwrap_err();
    match &e {
        Error::Graphql(m) => assert!(m.contains("no such field"), "{m}"),
        e => panic!("{e:?}"),
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    no_secret(&e);
}

#[test]
fn a_response_that_does_not_fit_the_operation_is_unexpected() {
    for body in [
        r#"{"nothing":"here"}"#,
        r#"{"data":{"findScenes":{"scenes":"not a list"}}}"#,
        "not json at all",
    ] {
        let (port, calls) = failing(200, body);
        let e = ids(&client_on(port)).unwrap_err();
        assert!(matches!(e, Error::UnexpectedResponse(_)), "{body}: {e:?}");
        assert_eq!(calls.load(Ordering::SeqCst), 1, "never retried");
        no_secret(&e);
    }
    let (port, _) = failing(418, "teapot");
    assert!(matches!(
        ids(&client_on(port)).unwrap_err(),
        Error::UnexpectedResponse(_)
    ));
}

#[test]
fn writes_are_never_retried() {
    let (port, calls) = failing(503, "");
    let e = client_on(port)
        .update_item_custom_fields(MediaKind::Scene, "1", serde_json::Map::new())
        .unwrap_err();
    assert!(matches!(e, Error::Unreachable(_)), "{e:?}");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
