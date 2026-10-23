# Feature Specification: Relations, Role Hierarchy, and Auto-Apply

**Feature Branch**: `004-relations-role-hierarchy`

**Created**: 2026-10-21

**Status**: Draft

**Input**: User description: "I want to make several modifications to the vocabulary now that we have a fast backed engine for analysis. I would like to add a new tag Relations. Relations are defined by roles can be autolinked when two performer are in an action together. I want to expand roles to be hierachial as well. I've included an example as a png diagram. I've included an auto apply example as well. When it hooks in for an update to a scene it should do the auto applies."

## Overview

The vocabulary (feature 001) has Roles, Labels, Actions, and pairings between two performers in
one scene. This feature adds three things the shared engine can now evaluate:

1. **Role hierarchy.** A role can be a narrower kind of another role. A narrower role takes part
   in everything its broader role takes part in.
2. **Relations.** A Relation is a new tag type that links two performers in one scene. It is
   defined by the two roles at its ends, and relations form a hierarchy of their own.
3. **Auto-apply.** A relation can carry rules that apply it automatically when two performers in
   a scene meet its conditions and are paired on an action. This happens whenever a scene is saved, with
   no one opening a client.

**The owner's examples.**

Example 1, hierarchy:
- **Roles:** Patient is a narrower role of Customer. Nurse and Doctor are narrower roles of
  Professional.
- **Relations:** Full Service links Customer and Professional. Extra Care links Patient and
  Nurse or Doctor, and is a narrower relation of Full Service.
- **Search:** a search for Full Service with "recursive" checked also finds scenes where a
  Patient and a Nurse or Doctor are linked by Extra Care.
- **Inherited participation:** a Patient can take part in every relation a Customer can.

Example 2, auto-apply:
- P1 has the Label Short and P2 has the Label Tall, and the two are paired on the Action Kissing
  in a scene.
- The relation Size Difference is then applied, with P1 at the Smaller end and P2 at the Larger
  end.
- Rules of this kind work from Labels or from Roles set in the scene, plus an Action linking the
  two performers.

## Clarifications

### Session 2026-10-21

- Q: Does sharing the same Action count as linking two performers? → A: No. Only an action pairing
  that joins the two performers links them. If they are not paired on the action, there is no way
  to know whether the relation applies.
- Q: Which recommendations should the engine provide in this feature? → A: Every category the
  plugin recommends today (Actions, Roles, Pluggables, Mise-en-scène), plus Relations, through one
  engine operation.
- Q: When should a Relation be recommended for a scene? → A: When it, or a common variant of it,
  is in the scene's native tag field, for example from Stash's Auto Tag or a scraper. The engine
  never writes native tags.

## User Scenarios & Testing *(mandatory)*

The users are the library owner, who defines the vocabulary and tags scenes, and the client
maintainers (the viewer and the semantic-tagging plugin), whose search screens use the engine.

### User Story 1 - Define role hierarchies and relations (Priority: P1)

The owner defines roles as narrower kinds of other roles, and defines relations by the roles at
their two ends, including narrower relations. The engine and every client then share one
definition of what each relation means.

**Why this priority**: Everything else rests on these definitions. Auto-apply needs relations to
apply, and recursive search needs the hierarchy to walk.

**Independent Test**: Define the roles and relations of Example 1 on the test library, then ask
the engine for the definitions. Patient's broader role is Customer. Extra Care's broader relation
is Full Service. Full Service's ends are Customer and Professional.

**Acceptance Scenarios**:

1. **Given** roles Customer and Patient, **When** the owner makes Patient a narrower role of
   Customer, **Then** the engine reports Customer as Patient's broader role. Patient takes part in
   every relation whose end is Customer.
2. **Given** roles Customer and Professional, **When** the owner defines the relation Full Service
   with ends Customer and Professional, **Then** the engine reports the relation with those two
   ends in that order.
3. **Given** Full Service, **When** the owner defines Extra Care as a narrower relation with ends
   Patient and Nurse or Doctor, **Then** the engine accepts it, because each end is the same role
   as, or a narrower role of, the matching end of Full Service.
4. **Given** a definition that loops (Customer narrower than Patient while Patient is narrower
   than Customer), **When** the engine reads it, **Then** it reports the loop and does not use
   the looping links. Every other definition keeps working.

---

### User Story 2 - Apply relations automatically when a scene is saved (Priority: P1)

When a scene is saved, the engine checks each relation's rules against that scene. Where two
performers meet a rule's conditions and are paired on a linking action, it writes the relation to the
scene in Stash. The owner does not have to open any client.

**Why this priority**: It is the main reason for the feature. Relations that must be tagged by
hand in every scene would rarely be complete.

**Independent Test**: On the test library, give P1 the Label Short and P2 the Label Tall, and tag
the two with the Action Kissing in one scene, then save. Within the stated time, the scene in
Stash shows the relation Size Difference with P1 at the Smaller end and P2 at the Larger end,
marked as applied automatically.

**Acceptance Scenarios**:

1. **Given** a rule "Size Difference when one end has Label Short and the other has Label Tall,
   linked by any Action", **When** a scene is saved where P1 (Short) and P2 (Tall) are paired on
   Kissing, **Then** Size Difference is applied with P1 at Smaller and P2 at Larger.
2. **Given** a rule "Full Service when one end has Role Customer and the other has Role
   Professional in the scene, linked by an Action", **When** a scene is saved where P1 has Role
   Patient, P2 has Role Doctor, and they are paired on an Action, **Then** Full Service is applied,
   because Patient is a narrower role of Customer and Doctor of Professional.
3. **Given** a scene where Size Difference was applied automatically, **When** the action pairing
   is removed and the scene saved, **Then** the automatic Size Difference is removed from the
   scene.
4. **Given** a scene where the owner applied Full Service by hand, **When** the scene is saved
   and the rule's conditions are not met, **Then** the hand-applied relation stays.
5. **Given** a scene whose relations are already up to date, **When** it is saved again without
   changes, **Then** nothing is written and no relation is duplicated.

---

### User Story 3 - Search relations and roles recursively (Priority: P2)

Search screens can ask for a relation or a role "recursively", so that narrower relations and
roles also match. Without recursion, only the exact tag matches.

**Why this priority**: Recursive search is what makes the hierarchy useful when browsing, but
definitions and auto-apply deliver value first.

**Independent Test**: On a library holding scenes with Full Service and scenes with Extra Care,
search for Full Service. Recursive returns both kinds. Non-recursive returns only Full Service.

**Acceptance Scenarios**:

1. **Given** scenes with Full Service and scenes with Extra Care, **When** a client searches for
   Full Service with recursive on, **Then** both kinds of scene are returned.
2. **Given** the same scenes, **When** the search has recursive off, **Then** only the Full
   Service scenes are returned.
3. **Given** performers with Role Patient, **When** a client searches for Role Customer with
   recursive on, **Then** scenes with those Patients match.

---

### User Story 4 - Recommend tags from the engine, including Relations (Priority: P2)

The plugin's tag pickers suggest vocabulary tags that a scene or image already carries among its
native Stash tags, for example tags added by a scraper. That logic moves into the engine (constitution
v3.0.0, Principles I and III), and Relations become one of the recommended categories.

**Why this priority**: Recommendations speed up tagging, but nothing else depends on them.

**Independent Test**: For fixture scenes whose native tags include vocabulary tags and common
variants, ask the engine for recommendations. They must match what the plugin recommends today for
Actions, Roles, Pluggables, and Mise-en-scène, and must include a Relation found among the native
tags.

**Acceptance Scenarios**:

1. **Given** a scene whose native tags include the Action tag Cowgirl, **When** a client asks for
   recommendations, **Then** Cowgirl is recommended under Actions.
2. **Given** a scene whose native tags include "Reverse Cowgirl", a common variant of Cowgirl,
   **When** a client asks, **Then** Cowgirl is recommended under Actions.
3. **Given** a scene whose native tags include the Relation tag Size Difference, **When** a client
   asks, **Then** Size Difference is recommended under Relations.
4. **Given** a scene with no vocabulary tags among its native tags, **When** a client asks,
   **Then** every category is empty.

### Edge Cases

- **Reversed direction:** P1 is Tall and P2 is Short. The relation is still applied, with each
  performer at the end whose condition they meet, so P2 is at Smaller.
- **Both directions qualify:** both performers meet both ends' conditions, for example both have
  Short and Tall. The relation is applied once per qualifying direction, and an end never holds
  the same performer twice.
- **Three or more performers:** every qualifying pair in the scene gets the relation.
- **A Relation in the tag field and also applied:** a Relation tag that Stash's Auto Tag put on
  the scene is recommended whether or not auto-apply has also applied it. A recommendation is a
  suggestion only; it never applies the relation.
- **Same Action, no pairing:** P1 (Short) and P2 (Tall) both hold Kissing but no action pairing
  joins them. Size Difference is not applied, because they may be kissing someone else.
- **Same performer at both ends:** not allowed. A performer is never linked to themselves.
- **Inputs to a rule change elsewhere:** a performer's Label changes, which happens when Labels
  are recomputed from Traits, or a rule or hierarchy is edited. Other scenes are not re-checked
  automatically. They are re-checked when next saved, or when the owner runs the re-apply task
  (FR-011).
- **An end role the performer already had by hand:** it is kept and not duplicated. Removing the
  automatic relation leaves the hand-set role in place (FR-008).
- **A relation or role tag is deleted or merged:** relation instances that used it are removed
  or moved with the tag, as Stash's own merge does.
- **The engine's own writes:** writing a relation to a scene saves that scene again, which fires
  another update. That second pass finds nothing to change and stops.
- **Engine absent or disabled when the scene is saved:** auto-apply runs when the engine next
  catches up (constitution Principle IV catch-up), not never.
- **Bulk edits:** a bulk edit of many scenes applies relations to all of them, within the bulk
  time limit in SC-005.
- **Images:** images carry the same role and action assignments as scenes and are treated the
  same way.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The vocabulary MUST add a Relation tag type alongside the existing tag types. A
  Relation names exactly two ends, each a Role, in a fixed order. A relation MAY name the same
  role at both ends, for a symmetric relation.
- **FR-002**: A Role MAY have one or more broader roles. A narrower role MUST take part in every
  relation whose end is any of its broader roles, at any depth.
- **FR-003**: A Relation MAY have one or more broader relations. Each end of a narrower relation
  MUST be the same role as, or a narrower role of, the matching end of each broader relation.
  Definitions that break this MUST be reported and not used.
- **FR-004**: Loops in the role or relation hierarchy MUST be reported, and the looping links MUST
  be ignored. Every other definition keeps working.
- **FR-005**: A relation instance MUST record its scene or image, the relation, which performer is
  at which end, and whether it was applied automatically or by hand.
- **FR-006**: A Relation MAY carry auto-apply rules. Each rule states:
  - a condition for each end: the performer has a given Label, or has a given Role in that scene
    or image (narrower roles count);
  - the linking action: an action pairing in that scene or image joins the two performers, on any
    Action or on one of a named set of Actions. Two performers who merely hold the same Action,
    without a pairing between them, are not linked.
- **FR-007**: When a scene or image is saved in Stash, the engine MUST evaluate the auto-apply
  rules for it. It MUST write the resulting relation instances to Stash, with no client action
  needed.
- **FR-008**: Applying a relation automatically MUST also assign each end's Role to the performer
  at that end, in that scene or image. For Size Difference, P1 gets Role Smaller and P2 gets Role
  Larger. Those role assignments are marked as automatic:
  - they are removed together with the relation (FR-009);
  - a Role the performer already had by hand in that scene is left alone and never duplicated.
- **FR-009**: An automatically applied relation, and the end roles it assigned, MUST be removed
  when its rule no longer holds for that scene or image. Hand-applied relations and hand-assigned
  roles MUST NOT be removed or duplicated by auto-apply.
- **FR-010**: Evaluating a scene or image whose relations are already correct MUST write nothing.
  The engine's own writes MUST NOT cause repeated evaluation beyond one pass that finds no
  change.
- **FR-011**: When a performer's Labels change, or when a rule or a hierarchy is edited, the engine
  MUST NOT re-evaluate other scenes by itself. The owner MUST be able to start a "re-apply
  relations" task that runs as a background job in Stash.
  - The task re-evaluates every scene and image, or a chosen subset, and reports its progress.
  - It can be cancelled (constitution Principle VII).
- **FR-012**: Auto-apply MUST NOT add more to a save in Stash than the hook budget allows. Today
  that budget is 7 ms per saved item (constitution Principle VII; 003 decision record rule 11).
  Evaluation and writing MUST run in a background job after the save completes. They MUST NOT be
  charged to the next search or other client call, which would delay it unpredictably: a
  1,000-scene edit would add seconds to whichever call came next.
- **FR-013**: Search criteria for a Relation or a Role MUST accept a "recursive" option. With it
  on, the criterion matches the named tag or any narrower one. With it off, which is the default,
  only the named tag matches.
- **FR-014**: Every automatically applied relation MUST be explainable on request: which rule
  applied it, and which Labels, Roles and Action met it (constitution Principle VII).
- **FR-015**: Relations, the role and relation hierarchies, auto-apply rules and relation instances
  MUST be stored in Stash (constitution Principle I), as documented tag flags and fields. The
  engine's store MUST be rebuildable from Stash alone.
- **FR-016**: The vocabulary MUST publish the new terms as a new minor version (1.2.0), with change
  log entries and the vocabulary checks passing. Existing terms keep their meaning.
- **FR-017**: Clients that do not know about Relations, such as the semantic-tagging plugin today,
  MUST keep working on a library that has them. The new data MUST NOT change the meaning of any
  existing flag or field.

- **FR-018**: The engine MUST answer, for one scene or image, the recommended tags per category:
  Actions, Roles, Pluggables, Mise-en-scène, and Relations. A tag of a category is recommended
  when it is among the item's native Stash tags. For Actions, Mise-en-scène, and Relations it is
  also recommended when one of those native tags is marked as a common variant of it
  (`variantOfTagId`); Roles and Pluggables match directly only, as in the plugin. This covers
  every category the plugin recommends today, with the same results, plus Relations.
- **FR-019**: Clients MUST be able to get recommendations from the engine alone, so the plugin
  can drop its own recommendation logic (constitution Principle III).
- **FR-020**: Recommendations MUST only read the item's native Stash tags. The engine MUST NOT add
  or remove native tags, for recommendations or for auto-apply: applied Relations live only in
  `relationAssignments` (FR-005).

### Key Entities

- **Role (extended)**: an existing Role tag, which can now have broader roles.
- **Relation**: a new tag type with two ordered ends, each a Role, and optional broader relations.
- **Auto-apply rule**: belongs to a Relation. It holds a condition per end (a Label, or a Role in
  the scene) and a linking action (an action pairing on any Action, or on named Actions).
- **Relation instance**: one Relation linking two performers in one scene or image. It records
  which performer is at which end and whether it was applied automatically or by hand.
- **Recommendation**: for one scene or image, a category (Action, Role, Pluggable, Mise-en-scène,
  Relation) and the tags suggested in it, each with the native tag that suggested it.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Both of the owner's examples can be expressed in full: every role, relation, link,
  and rule in the two diagrams.
- **SC-002**: After a scene that meets a rule is saved, the relation is visible in Stash within 30
  seconds, without any client being opened.
- **SC-003**: Installing this feature adds no more than 7 ms per saved item to a save in Stash.
- **SC-004**: On a test library with known answers:
  - a recursive search for Full Service returns exactly the scenes with Full Service or Extra
    Care;
  - a non-recursive search returns exactly the Full Service scenes;
  - zero scenes are wrong in either direction.
- **SC-005**: After a bulk edit of 1,000 scenes, every resulting auto-apply is written to Stash
  within 5 minutes.
- **SC-006**: Saving an unchanged scene 10 times in a row produces no duplicate relations and no
  writes after the first pass.
- **SC-007**: Recursive searches answer in under 200 ms for 95% of searches on a library of
  250,000 scenes, the same budget as other queries.
- **SC-008**: On the fixture library, the engine's recommendations for Actions, Roles, Pluggables,
  and Mise-en-scène equal the plugin's current recommendations for 100% of items. Each
  recommendation request answers in under 200 ms.

## Assumptions

- **Hierarchy storage**: role and relation hierarchies use Stash's own tag parent links (the
  vocabulary's existing `parentTag`), not a new field. Stash already lets a tag have several
  parents.
- **Relation storage**: a Relation is a flagged tag, like the other tag types (for example a new
  `isRelation` flag), with its two ends and its rules in the tag's custom fields. Relation
  instances live in the scene's or image's custom fields, as role and action assignments do today.
  The exact keys are set in the plan.
- **Design documents**: the semantic-tagging plugin's documents (read at commit `3c82c85`) have no
  Relations, role hierarchy, or auto-apply.
  - Under constitution Principle II this is a deliberate, documented addition. It is additive, so
    no migration is needed.
  - The new terms are sent to the plugin so its documents can adopt them.
- **Labels**: Labels keep working as today. A rule only reads a performer's current Labels.
- **Engine phases**: this feature builds on the engine's semantic graph and hook-driven updates
  (ROADMAP Phase 2) and on query operations (Phase 3). The vocabulary terms (FR-001 to FR-006 and
  FR-016) can be published first. Auto-apply and recursive search need those phases.
- **Background jobs**:
  - Applying changes runs as a Stash background job (task), started by the hook after it records
    the change, and so does the re-apply task. Neither saves nor client calls pay for it (FR-011,
    FR-012).
  - This replaces the 003 decision record's rule 6, "the next engine call applies". The owner
    judged that a 1,000-scene edit adding seconds to some later call at random is not acceptable.
  - The plan updates rule 6 and measures the hook's cost with the job request added.
- **Who sees relations**: relations appear in Stash as data. How each client displays them is up
  to that client, which is a user interface over the engine (constitution v3.0.0).
