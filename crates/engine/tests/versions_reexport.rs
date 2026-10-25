//! The engine reports the adapter's minimum Stash version (spec 005 T004).

#[test]
fn the_engine_reexports_the_adapters_values() {
    assert_eq!(
        engine::STASH_MINIMUM_VERSION,
        stash_adapter::version::MINIMUM_STASH_VERSION
    );
    assert!(engine::versions::stash_supported("v0.31.1"));
    assert!(!engine::versions::stash_supported("v0.30.0"));
}
