use semantic_vocabulary::iri::{IriError, ItemIris};

const SERVER: &str = "00000000-0000-4000-8000-000000000001";

fn iris() -> ItemIris {
    ItemIris::new(SERVER).unwrap()
}

#[test]
fn entity_iris_match_the_scheme() {
    let i = iris();
    let p = "urn:semantic-stash:00000000-0000-4000-8000-000000000001";
    assert_eq!(i.scene("12"), format!("{p}:scene:12"));
    assert_eq!(i.image("7"), format!("{p}:image:7"));
    assert_eq!(i.performer("3"), format!("{p}:performer:3"));
    assert_eq!(i.tag("99"), format!("{p}:tag:99"));
    assert_eq!(i.marker("5"), format!("{p}:marker:5"));
}

#[test]
fn derived_iris_match_the_scheme() {
    let i = iris();
    let m = i.scene("12");
    assert_eq!(i.role_instance(&m, "3", "40"), format!("{m}/role:3:40"));
    assert_eq!(i.action_instance(&m, "3", "41"), format!("{m}/action:3:41"));
    assert_eq!(
        i.pluggable_instance(&m, "3", "42"),
        format!("{m}/pluggable:3:42")
    );
    assert_eq!(i.mise_en_scene_instance(&m, "50"), format!("{m}/mise:50"));
    assert_eq!(i.pairing(&m, "pair-1"), format!("{m}/pairing:pair-1"));
    assert_eq!(i.timeline(&m), format!("{m}/timeline"));
    let e = i.timeline_entry(&m, "entry-1");
    assert_eq!(e, format!("{m}/timeline/entry-1"));
    assert_eq!(i.stage_pick(&e, "50"), format!("{e}/pick:50"));
    let l = i.tag("88");
    assert_eq!(i.label_rule(&l, "r1"), format!("{l}/rule:r1"));
}

#[test]
fn building_twice_is_identical() {
    let a = iris();
    let b = iris();
    assert_eq!(a.scene("12"), b.scene("12"));
    assert_eq!(
        a.role_instance(&a.scene("12"), "3", "40"),
        b.role_instance(&b.scene("12"), "3", "40")
    );
}

#[test]
fn rejects_malformed_server_and_ids() {
    assert!(matches!(
        ItemIris::new("not-a-uuid"),
        Err(IriError::BadServerId(_))
    ));
    assert!(matches!(
        ItemIris::new("00000000-0000-4000-8000-00000000000A"),
        Err(IriError::BadServerId(_))
    ));
    let i = iris();
    assert!(i.try_scene("1:2").is_err());
    assert!(i.try_scene("a/b").is_err());
    assert!(i.try_scene("").is_err());
}

#[test]
fn relation_iris_match_the_scheme() {
    let i = iris();
    let m = i.scene("12");
    // Ordered ends keep the two directions of one relation distinct.
    assert_eq!(
        i.relation_instance(&m, "45", "7", "8"),
        format!("{m}/relation:45:7:8")
    );
    assert_ne!(
        i.relation_instance(&m, "45", "7", "8"),
        i.relation_instance(&m, "45", "8", "7")
    );
    let r = i.tag("45");
    assert_eq!(i.relation_rule(&r, "r1"), format!("{r}/rule:r1"));
}
