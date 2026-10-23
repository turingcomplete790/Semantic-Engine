//! Tag recommendations for one scene or image (spec 004 FR-018 to FR-020; research R13).
//!
//! A port of the semantic-tagging plugin's `computeRecommendedTagIds` (`src/ui/shared.js` at
//! `3c82c85`): a category's tag is recommended when it is one of the item's native tags, or, for
//! the categories the plugin resolves through common variants, when a native tag's
//! `variantOfTagId` points at it. The plugin resolves variants for Actions and Mise-en-scène
//! only; Roles and Pluggables match directly. Relations (new in 004) resolve variants like
//! Actions. Recommendations only read; they never write native tags (FR-020).

use crate::relations::defs::Definitions;
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Recommendation {
    /// The recommended vocabulary tag.
    pub tag: String,
    /// The native tag on the item that suggested it: the tag itself, or one of its variants.
    pub from: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recommendations {
    pub actions: Vec<Recommendation>,
    pub roles: Vec<Recommendation>,
    pub pluggables: Vec<Recommendation>,
    pub mise_en_scene: Vec<Recommendation>,
    pub relations: Vec<Recommendation>,
}

fn category(
    native: &[String],
    d: &Definitions,
    members: &dyn Fn(&str) -> bool,
    variants: bool,
) -> Vec<Recommendation> {
    let mut out = BTreeSet::new();
    for n in native {
        if members(n) {
            out.insert(Recommendation {
                tag: n.clone(),
                from: n.clone(),
            });
        } else if variants {
            if let Some(c) = d.variant_of(n).filter(|c| members(c)) {
                out.insert(Recommendation {
                    tag: c.to_string(),
                    from: n.clone(),
                });
            }
        }
    }
    out.into_iter().collect()
}

/// The recommendations for an item whose native tag ids are `native`.
pub fn recommend(native: &[String], d: &Definitions) -> Recommendations {
    Recommendations {
        actions: category(native, d, &|t| d.actions.contains(t), true),
        roles: category(native, d, &|t| d.roles.contains(t), false),
        pluggables: category(native, d, &|t| d.pluggables.contains(t), false),
        mise_en_scene: category(native, d, &|t| d.mise_en_scene.contains(t), true),
        relations: category(native, d, &|t| d.relations.contains_key(t), true),
    }
}
