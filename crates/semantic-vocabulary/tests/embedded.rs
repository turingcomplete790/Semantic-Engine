mod common;
use semantic_vocabulary::{embedded, load};

#[test]
fn embedded_equals_disk() {
    let disk = load(&common::repo_vocab_dir()).unwrap();
    let emb = embedded().unwrap();
    let key = |v: &semantic_vocabulary::Vocabulary| {
        let mut t: Vec<String> = v
            .files
            .iter()
            .flat_map(|f| f.triples.iter().map(|t| format!("{} {t}", f.name)))
            .collect();
        t.sort();
        t
    };
    assert_eq!(key(&disk), key(&emb));
}
