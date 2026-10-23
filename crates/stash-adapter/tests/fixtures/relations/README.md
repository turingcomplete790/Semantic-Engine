# Recorded responses for relation reads and writes (spec 004, task T003)

Hand-written in the exact shape Stash v0.31.1 returns for the adapter's typed operations,
generated from `crates/engine/tests/fixtures/relations/` so the adapter and engine fixtures
describe the same library. No live tags of these kinds exist yet to capture read-only; no
credentials appear.

- `tags_<flag>.json`: `findTags` filtered by `<flag>` NOT_NULL, one file per flag.
- `items_scenes.json`, `items_images.json`: `findScenes` / `findImages` by ids.
- `performers.json`: `findPerformers` by ids, with tag ids.
- `changed_since_scenes.json`: scene ids with `updated_at` after a threshold.
- `native_tags_scene.json`, `native_tags_missing.json`: one scene's native tags, and a missing
  scene.
- `update_scene.json`, `run_plugin_task.json`: replies to the two writes.
