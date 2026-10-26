//! Build notes: one skipped or mismatched field or entry (data-model § Note; research R8). A
//! note says where the problem is, never what the field held, because values can hold names.

use serde::Serialize;
use stash_adapter::reader::Kind;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NoteReason {
    /// The reader marked the field unreadable.
    Unreadable,
    /// The value, or one entry of it, is not the documented shape.
    BadShape,
    /// An id is not a valid Stash id, or is refused by the item-IRI scheme.
    InvalidId,
    /// A timeline entry's `ref.kind` is not one the documents define.
    UnknownKind,
    /// A retired key or form.
    Retired,
    /// A slot or pairing names an assignment or pairing not in the record.
    DanglingRef,
    /// A referenced tag is not among the reader's tags; mapped anyway.
    TagMissing,
    /// A referenced tag lacks the expected flag; mapped anyway.
    TagWrongType,
    /// An assignment names a performer not on the media item; mapped anyway.
    PerformerNotInMedia,
    /// An automatic role that is not in `roleTagAssignments`; mapped anyway.
    NotRecordedRole,
}

impl NoteReason {
    /// True for the reasons that skip data; false for mismatches, which are mapped.
    pub fn is_skip(self) -> bool {
        matches!(
            self,
            NoteReason::Unreadable
                | NoteReason::BadShape
                | NoteReason::InvalidId
                | NoteReason::UnknownKind
                | NoteReason::Retired
                | NoteReason::DanglingRef
        )
    }

    /// The contract's camelCase name.
    pub fn as_str(self) -> &'static str {
        match self {
            NoteReason::Unreadable => "unreadable",
            NoteReason::BadShape => "badShape",
            NoteReason::InvalidId => "invalidId",
            NoteReason::UnknownKind => "unknownKind",
            NoteReason::Retired => "retired",
            NoteReason::DanglingRef => "danglingRef",
            NoteReason::TagMissing => "tagMissing",
            NoteReason::TagWrongType => "tagWrongType",
            NoteReason::PerformerNotInMedia => "performerNotInMedia",
            NoteReason::NotRecordedRole => "notRecordedRole",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Note {
    pub kind: Kind,
    pub id: String,
    pub key: String,
    /// The entry the note is about: a performer id, a list position, an entry or rule id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
    pub reason: NoteReason,
}

impl Note {
    /// One log line, for example `note: scene 12 roleTagAssignments [7]: tagWrongType`.
    pub fn log_line(&self) -> String {
        let entry = self
            .entry
            .as_ref()
            .map(|e| format!(" [{e}]"))
            .unwrap_or_default();
        format!(
            "note: {} {} {}{entry}: {}",
            self.kind.as_str(),
            self.id,
            self.key,
            self.reason.as_str()
        )
    }
}

/// Notes in the order found, each kept once.
#[derive(Debug, Clone, Default)]
pub struct Notes {
    list: Vec<Note>,
    seen: HashSet<Note>,
}

impl Notes {
    pub fn push(&mut self, note: Note) {
        if self.seen.insert(note.clone()) {
            self.list.push(note);
        }
    }

    pub fn extend(&mut self, other: Notes) {
        for n in other.list {
            self.push(n);
        }
    }

    pub fn as_slice(&self) -> &[Note] {
        &self.list
    }

    pub fn into_vec(self) -> Vec<Note> {
        self.list
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(entry: Option<&str>, reason: NoteReason) -> Note {
        Note {
            kind: Kind::Scene,
            id: "12".into(),
            key: "roleTagAssignments".into(),
            entry: entry.map(str::to_string),
            reason,
        }
    }

    #[test]
    fn equal_notes_are_kept_once_in_order() {
        let mut n = Notes::default();
        n.push(note(Some("7"), NoteReason::TagWrongType));
        n.push(note(None, NoteReason::Unreadable));
        n.push(note(Some("7"), NoteReason::TagWrongType));
        assert_eq!(n.len(), 2);
        assert_eq!(n.as_slice()[1].reason, NoteReason::Unreadable);
    }

    #[test]
    fn a_log_line_names_the_place_and_reason_only() {
        assert_eq!(
            note(Some("7"), NoteReason::TagWrongType).log_line(),
            "note: scene 12 roleTagAssignments [7]: tagWrongType"
        );
        assert_eq!(
            note(None, NoteReason::Unreadable).log_line(),
            "note: scene 12 roleTagAssignments: unreadable"
        );
    }

    #[test]
    fn skips_and_mismatches_are_told_apart() {
        assert!(NoteReason::DanglingRef.is_skip());
        assert!(!NoteReason::TagMissing.is_skip());
        assert_eq!(
            serde_json::to_value(NoteReason::PerformerNotInMedia).unwrap(),
            "performerNotInMedia"
        );
    }
}
