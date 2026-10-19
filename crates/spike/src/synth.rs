//! Vocabulary-shaped synthetic graphs (research R8, tasks T011).
//!
//! The shape mirrors what `spike-bench seed` writes to the test library, so synthetic figures
//! and real ones measure the same kind of graph. Per scene:
//!
//! - 2 performers (`hasPerformer`),
//! - per performer 1 role instance and 1 action instance (with one variation string), and a
//!   pluggable instance for the first performer only,
//!
//! which matches the seed's per-scene densities (`bench::seed::SCENE_SHAPE`). Images: 1.3 per
//! scene (the test library's 1,327 images to 1,021 scenes), each with 1 performer and 1 role
//! instance. Performers: one per 50 scenes, at least 6 (the test library's count), each with
//! one Trait and one Label. Tags: 6 per flagged type (`plugtag`, `isAction`, `isRole`, `isTrait`,
//! `isLabel`, `isMiseEnScene`, `isVariation`), as the seed creates.
//!
//! Quads are generated lazily so a 250,000-scene graph streams into the bulk loader.

use oxigraph::model::{GraphName, Literal, NamedNode, Quad, Subject, Term};
use oxigraph::store::Store;
use semantic_vocabulary::iri::ItemIris;

pub const VOCAB: &str = "urn:semantic-stash:vocab:";
pub const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
pub const RDFS_LABEL: &str = "http://www.w3.org/2000/01/rdf-schema#label";
/// The example server identifier the vocabulary documents use.
pub const SERVER_ID: &str = "00000000-0000-4000-8000-000000000001";
/// Tags per flagged type, as the seed creates them.
pub const TAGS_PER_TYPE: u64 = 6;
/// The flagged tag types and the vocabulary class each one maps to.
pub const TAG_TYPES: [(&str, &str); 7] = [
    ("plugtag", "Pluggable"),
    ("isAction", "Action"),
    ("isRole", "Role"),
    ("isTrait", "Trait"),
    ("isLabel", "Label"),
    ("isMiseEnScene", "MiseEnScene"),
    ("isVariation", "Variation"),
];
/// The test library's scene count (Stash `stats` on 2026-10-16).
pub const TEST_SCENES: u64 = 1021;

/// Parses `test`, `50k`, `100k`, `250k`, or a plain scene count.
pub fn parse_scale(s: &str) -> Option<u64> {
    match s {
        "test" => Some(TEST_SCENES),
        _ => match s.strip_suffix('k') {
            Some(n) => n.parse::<u64>().ok().map(|n| n * 1000),
            None => s.parse().ok(),
        },
    }
    .filter(|n| *n > 0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spec {
    pub scenes: u64,
    pub seed: u64,
}

impl Spec {
    pub fn new(scenes: u64) -> Spec {
        Spec { scenes, seed: 1 }
    }
    pub fn images(&self) -> u64 {
        self.scenes * 1327 / 1021
    }
    pub fn performers(&self) -> u64 {
        (self.scenes / 50).max(6)
    }
    pub fn tags(&self) -> u64 {
        TAGS_PER_TYPE * TAG_TYPES.len() as u64
    }
}

/// Deterministic generator (SplitMix64), so the same spec always gives the same quads.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn nn(iri: &str) -> NamedNode {
    NamedNode::new_unchecked(iri)
}
fn v(local: &str) -> NamedNode {
    nn(&format!("{VOCAB}{local}"))
}
fn quad(s: &str, p: NamedNode, o: impl Into<Term>) -> Quad {
    Quad::new(
        Subject::NamedNode(nn(s)),
        p,
        o.into(),
        GraphName::DefaultGraph,
    )
}

/// Tag id for type index `t` (0..7) and number `i` (0..6): ids 1001.. keep clear of real ones.
pub fn tag_id(t: usize, i: u64) -> String {
    (1001 + t as u64 * TAGS_PER_TYPE + i).to_string()
}

/// All quads for `spec`, generated lazily in a fixed order.
pub fn quads(spec: Spec) -> impl Iterator<Item = Quad> {
    let iris = ItemIris::new(SERVER_ID).expect("valid example server id");
    let tags = tag_quads(&iris);
    let performers = {
        let iris = iris.clone();
        let mut rng = Rng(spec.seed ^ 0x5045_5246);
        (0..spec.performers()).flat_map(move |p| performer_quads(&iris, &mut rng, p))
    };
    let scenes = {
        let iris = iris.clone();
        let mut rng = Rng(spec.seed ^ 0x5343_454E);
        let np = spec.performers();
        (0..spec.scenes).flat_map(move |s| scene_quads(&iris, &mut rng, s, np))
    };
    let images = {
        let mut rng = Rng(spec.seed ^ 0x494D_4147);
        let np = spec.performers();
        (0..spec.images()).flat_map(move |i| image_quads(&iris, &mut rng, i, np))
    };
    tags.into_iter()
        .chain(performers)
        .chain(scenes)
        .chain(images)
}

fn tag_quads(iris: &ItemIris) -> Vec<Quad> {
    let mut out = Vec::new();
    for (t, (flag, class)) in TAG_TYPES.iter().enumerate() {
        for i in 0..TAGS_PER_TYPE {
            let tag = iris.tag(&tag_id(t, i));
            out.push(quad(&tag, nn(RDF_TYPE), v(class)));
            out.push(quad(
                &tag,
                nn(RDFS_LABEL),
                Literal::new_simple_literal(format!("SPIKE {flag} {i}")),
            ));
        }
    }
    out
}

fn type_tag(rng: &mut Rng, class: &str) -> String {
    let t = TAG_TYPES
        .iter()
        .position(|(_, c)| *c == class)
        .expect("known class");
    tag_id(t, rng.below(TAGS_PER_TYPE))
}

fn performer_quads(iris: &ItemIris, rng: &mut Rng, p: u64) -> Vec<Quad> {
    let perf = iris.performer(&(p + 1).to_string());
    vec![
        quad(&perf, nn(RDF_TYPE), v("Performer")),
        quad(
            &perf,
            nn(RDFS_LABEL),
            Literal::new_simple_literal(format!("Performer {}", p + 1)),
        ),
        quad(&perf, v("hasTrait"), nn(&iris.tag(&type_tag(rng, "Trait")))),
        quad(&perf, v("hasLabel"), nn(&iris.tag(&type_tag(rng, "Label")))),
    ]
}

/// Adds one reified scene-scoped instance (`role`, `action`, or `pluggable`) of `tag` for a
/// performer in a media item: the performer link, the type, and its media, performer, and tag.
pub fn instance(
    out: &mut Vec<Quad>,
    iris: &ItemIris,
    media: &str,
    perf_id: &str,
    kind: &str,
    tag: &str,
    variation: Option<&str>,
) {
    let perf = iris.performer(perf_id);
    let (inst, class, link) = match kind {
        "role" => (
            iris.role_instance(media, perf_id, tag),
            "RoleInstance",
            "hasRole",
        ),
        "action" => (
            iris.action_instance(media, perf_id, tag),
            "ActionInstance",
            "hasAction",
        ),
        _ => (
            iris.pluggable_instance(media, perf_id, tag),
            "PluggableInstance",
            "hasPluggable",
        ),
    };
    out.push(quad(&perf, v(link), nn(&inst)));
    out.push(quad(&inst, nn(RDF_TYPE), v(class)));
    out.push(quad(&inst, v("inMedia"), nn(media)));
    out.push(quad(&inst, v("forPerformer"), nn(&perf)));
    out.push(quad(&inst, v("ofTag"), nn(&iris.tag(tag))));
    if let Some(var) = variation {
        out.push(quad(
            &inst,
            v("hasVariation"),
            Literal::new_simple_literal(var),
        ));
    }
}

const VARIATIONS: [&str; 4] = ["slow", "fast", "playful", "intense"];

fn scene_quads(iris: &ItemIris, rng: &mut Rng, s: u64, performers: u64) -> Vec<Quad> {
    let scene = iris.scene(&(s + 1).to_string());
    let mut out = vec![quad(&scene, nn(RDF_TYPE), v("Scene"))];
    let a = rng.below(performers) + 1;
    let b = (a % performers) + 1;
    for (k, p) in [a, b].into_iter().enumerate() {
        let pid = p.to_string();
        out.push(quad(&scene, v("hasPerformer"), nn(&iris.performer(&pid))));
        let role = type_tag(rng, "Role");
        instance(&mut out, iris, &scene, &pid, "role", &role, None);
        let action = type_tag(rng, "Action");
        let var = VARIATIONS[rng.below(VARIATIONS.len() as u64) as usize];
        instance(&mut out, iris, &scene, &pid, "action", &action, Some(var));
        if k == 0 {
            let plug = type_tag(rng, "Pluggable");
            instance(&mut out, iris, &scene, &pid, "pluggable", &plug, None);
        }
    }
    out
}

fn image_quads(iris: &ItemIris, rng: &mut Rng, i: u64, performers: u64) -> Vec<Quad> {
    let image = iris.image(&(i + 1).to_string());
    let pid = (rng.below(performers) + 1).to_string();
    let mut out = vec![
        quad(&image, nn(RDF_TYPE), v("Image")),
        quad(&image, v("hasPerformer"), nn(&iris.performer(&pid))),
    ];
    let role = type_tag(rng, "Role");
    instance(&mut out, iris, &image, &pid, "role", &role, None);
    out
}

/// Streams `spec` into `store` with the bulk loader and returns the number of quads loaded.
pub fn load(store: &Store, spec: Spec) -> Result<u64, String> {
    let mut n = 0u64;
    let counted = quads(spec).inspect(|_| n += 1);
    store
        .bulk_loader()
        .load_quads(counted)
        .map_err(|e| e.to_string())?;
    Ok(n)
}

/// The synthetic tag id of the `i`-th tag of a class (`Role`, `Action`, ...).
pub fn tag_id_for(class: &str, i: u64) -> String {
    let t = TAG_TYPES
        .iter()
        .position(|(_, c)| *c == class)
        .expect("known class");
    tag_id(t, i % TAGS_PER_TYPE)
}

/// The vocabulary class for a tag flag (`isRole` -> `Role`).
pub fn class_for_flag(flag: &str) -> Option<&'static str> {
    TAG_TYPES.iter().find(|(f, _)| *f == flag).map(|(_, c)| *c)
}

/// Quad helpers for code that maps real Stash data with the same shape.
pub fn typed(subject: &str, class: &str) -> Quad {
    quad(subject, nn(RDF_TYPE), v(class))
}
pub fn link(subject: &str, property: &str, object: &str) -> Quad {
    quad(subject, v(property), nn(object))
}
pub fn label(subject: &str, text: &str) -> Quad {
    quad(subject, nn(RDFS_LABEL), Literal::new_simple_literal(text))
}
