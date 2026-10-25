//! No Oxigraph type in the graph module's public interface (spec 006 T015; rule G9, FR-001).

#[test]
fn no_public_item_mentions_oxigraph() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/graph");
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        for (i, line) in text.lines().enumerate() {
            let l = line.trim_start();
            let public = l.starts_with("pub ") && !l.starts_with("pub(");
            if public {
                checked += 1;
                assert!(
                    !l.contains("oxigraph") && !l.contains("ox::"),
                    "{}:{}: {l}",
                    path.display(),
                    i + 1
                );
            }
        }
    }
    assert!(checked > 20, "only {checked} public lines found");
}
