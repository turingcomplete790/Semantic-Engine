//! Stash version parsing and the minimum supported version (spec 005 T004, FR-017).

use stash_adapter::version::{parse_stash_version, stash_supported, MINIMUM_STASH_VERSION};

#[test]
fn versions_parse_with_or_without_a_leading_v_and_a_suffix() {
    assert_eq!(parse_stash_version("v0.31.1"), Some((0, 31, 1)));
    assert_eq!(parse_stash_version("0.31.1"), Some((0, 31, 1)));
    assert_eq!(parse_stash_version("v0.31.1-12-gabcdef"), Some((0, 31, 1)));
    assert_eq!(parse_stash_version(" v1.2.3+build "), Some((1, 2, 3)));
    assert_eq!(parse_stash_version("v0.31"), None);
    assert_eq!(parse_stash_version("nonsense"), None);
    assert_eq!(parse_stash_version(""), None);
}

#[test]
fn only_the_minimum_and_newer_are_supported() {
    assert_eq!(MINIMUM_STASH_VERSION, "v0.31.1");
    assert!(stash_supported("v0.31.1"));
    assert!(stash_supported("v0.31.2"));
    assert!(stash_supported("v0.32.0"));
    assert!(stash_supported("v1.0.0"));
    assert!(!stash_supported("v0.31.0"));
    assert!(!stash_supported("v0.30.9"));
    assert!(!stash_supported("nonsense"));
}
