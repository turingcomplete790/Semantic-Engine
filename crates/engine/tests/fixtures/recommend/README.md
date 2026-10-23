# Recommendation parity fixtures (spec 004, task T035)

The expected values reproduce the semantic-tagging plugin's `computeRecommendedTagIds`
(`src/ui/shared.js`) and its call sites, read at plugin commit `3c82c85` on 2026-10-23:

- `actions.js:976`: Actions, direct or through a common variant (`buildCanonicalIdByVariantId`).
- `timeline.js:358`: Mise-en-scène, direct or through a common variant.
- `roles.js:554`: Roles, direct only (an empty variant map).
- `pluggables.js:499`: Pluggables, direct only (an empty variant map).

A tag is recommended when it is one of the item's native tags; otherwise a native tag that is a
common variant (`variantOfTagId`) recommends the tag it points at, if that tag belongs to the
category. Relations are new in spec 004 and use the Action rule (FR-018). Item 4 checks that a
variant pointing at a Role is not resolved, as in the plugin.
