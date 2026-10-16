use oxigraph::model::{GraphNameRef, LiteralRef, NamedNodeRef, QuadRef};
use oxigraph::store::Store;

#[test]
fn bundled_rocksdb_store_opens_writes_and_reads() {
    let dir = std::env::temp_dir().join(format!("se-store-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    {
        let store = Store::open(&dir).expect("bundled RocksDB store opens");
        let s = NamedNodeRef::new("urn:semantic-stash:test:s").unwrap();
        let p = NamedNodeRef::new("urn:semantic-stash:test:p").unwrap();
        store
            .insert(QuadRef::new(
                s,
                p,
                LiteralRef::new_simple_literal("o"),
                GraphNameRef::DefaultGraph,
            ))
            .unwrap();
        assert_eq!(store.len().unwrap(), 1);
    }
    let reopened = Store::open(&dir).unwrap();
    assert_eq!(reopened.len().unwrap(), 1, "data persists across reopen");
    drop(reopened);
    std::fs::remove_dir_all(&dir).unwrap();
}
