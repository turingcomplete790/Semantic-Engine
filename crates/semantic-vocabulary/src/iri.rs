//! Item IRI scheme (FR-014): `urn:semantic-stash:{server}:{type}:{id}` and derived nodes.

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum IriError {
    #[error("server identifier must be a lower-case hyphenated UUID, got {0:?}")]
    BadServerId(String),
    #[error("identifier {0:?} must be non-empty and contain only letters, digits, '-' or '_'")]
    BadId(String),
}

/// Builds item IRIs for one server. The server identifier is the UUID stored in Stash.
#[derive(Debug, Clone)]
pub struct ItemIris {
    server: String,
}

fn valid_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    let lens = [8, 4, 4, 4, 12];
    parts.len() == 5
        && parts.iter().zip(lens).all(|(p, n)| {
            p.len() == n
                && p.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
}

fn check_id(id: &str) -> Result<&str, IriError> {
    if !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        Ok(id)
    } else {
        Err(IriError::BadId(id.to_string()))
    }
}

impl ItemIris {
    pub fn new(server: &str) -> Result<Self, IriError> {
        if valid_uuid(server) {
            Ok(Self {
                server: server.to_string(),
            })
        } else {
            Err(IriError::BadServerId(server.to_string()))
        }
    }

    fn entity(&self, kind: &str, id: &str) -> Result<String, IriError> {
        Ok(format!(
            "urn:semantic-stash:{}:{}:{}",
            self.server,
            kind,
            check_id(id)?
        ))
    }

    pub fn try_scene(&self, id: &str) -> Result<String, IriError> {
        self.entity("scene", id)
    }
    pub fn try_image(&self, id: &str) -> Result<String, IriError> {
        self.entity("image", id)
    }
    pub fn try_performer(&self, id: &str) -> Result<String, IriError> {
        self.entity("performer", id)
    }
    pub fn try_tag(&self, id: &str) -> Result<String, IriError> {
        self.entity("tag", id)
    }
    pub fn try_marker(&self, id: &str) -> Result<String, IriError> {
        self.entity("marker", id)
    }

    // The infallible forms panic on a malformed id; callers hold Stash-issued ids.
    pub fn scene(&self, id: &str) -> String {
        self.try_scene(id).expect("valid id")
    }
    pub fn image(&self, id: &str) -> String {
        self.try_image(id).expect("valid id")
    }
    pub fn performer(&self, id: &str) -> String {
        self.try_performer(id).expect("valid id")
    }
    pub fn tag(&self, id: &str) -> String {
        self.try_tag(id).expect("valid id")
    }
    pub fn marker(&self, id: &str) -> String {
        self.try_marker(id).expect("valid id")
    }

    fn instance(&self, media: &str, kind: &str, performer: &str, tag: &str) -> String {
        format!(
            "{media}/{kind}:{}:{}",
            check_id(performer).expect("valid"),
            check_id(tag).expect("valid")
        )
    }
    pub fn role_instance(&self, media: &str, performer: &str, tag: &str) -> String {
        self.instance(media, "role", performer, tag)
    }
    pub fn action_instance(&self, media: &str, performer: &str, tag: &str) -> String {
        self.instance(media, "action", performer, tag)
    }
    pub fn pluggable_instance(&self, media: &str, performer: &str, tag: &str) -> String {
        self.instance(media, "pluggable", performer, tag)
    }
    pub fn mise_en_scene_instance(&self, media: &str, tag: &str) -> String {
        format!("{media}/mise:{}", check_id(tag).expect("valid"))
    }
    pub fn pairing(&self, media: &str, pairing_id: &str) -> String {
        format!("{media}/pairing:{}", check_id(pairing_id).expect("valid"))
    }
    pub fn timeline(&self, media: &str) -> String {
        format!("{media}/timeline")
    }
    pub fn timeline_entry(&self, media: &str, entry_id: &str) -> String {
        format!("{media}/timeline/{}", check_id(entry_id).expect("valid"))
    }
    pub fn stage_pick(&self, entry: &str, tag: &str) -> String {
        format!("{entry}/pick:{}", check_id(tag).expect("valid"))
    }
    /// `rule_id` is the rule's own stable id inside the Label formula, not its position.
    pub fn label_rule(&self, label: &str, rule_id: &str) -> String {
        format!("{label}/rule:{}", check_id(rule_id).expect("valid"))
    }
    /// One relation between two performers in one media item; `a` and `b` are the performers at
    /// the relation's first and second end, so the two directions get different IRIs (since 1.2.0).
    pub fn relation_instance(&self, media: &str, relation: &str, a: &str, b: &str) -> String {
        format!(
            "{media}/relation:{}:{}:{}",
            check_id(relation).expect("valid"),
            check_id(a).expect("valid"),
            check_id(b).expect("valid")
        )
    }
    /// An auto-apply rule of a relation, by the rule's own stable id (since 1.2.0).
    pub fn relation_rule(&self, relation: &str, rule_id: &str) -> String {
        format!("{relation}/rule:{}", check_id(rule_id).expect("valid"))
    }
    /// One Field Mapping slot of a Trait, Label or Variation tag (since 1.3.0).
    pub fn field_mapping(&self, tag: &str, slot: FieldMappingSlot) -> String {
        match slot {
            FieldMappingSlot::Base => format!("{tag}/fieldMapping:base"),
            FieldMappingSlot::Variation(n) => format!("{tag}/fieldMapping:variation:{n}"),
        }
    }
}

/// Which Field Mapping slot of a tag an IRI names (since 1.3.0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldMappingSlot {
    /// The tag's own slot.
    Base,
    /// A Trait's slot for one declared variation, by the variation's index in the sorted
    /// variation names: names are free text and not valid IRI ids.
    Variation(usize),
}
