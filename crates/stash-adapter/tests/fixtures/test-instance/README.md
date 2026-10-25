# Test-instance fixtures

Captured with `fixture-capture` from the local Stash v0.31.1 test instance on 2026-10-25. Since
spec 005 the capture runs through the library reader, so these are reader records, not raw rows.
It sent 40 read requests (`index.json`).

**Checks:**
- The library counts were identical before and after the capture: 1,021 scenes, 1,327 images,
  1,107 performers and 22 tags.
- No credential is recorded.

**What the read found:**
- **Tags:** 17 semantic tags.
- **Media:** 1 scene and 1 image with semantic data.
- **Performers:** 886 semantic performers. Most of them have a `measurements` field (791 of the
  1,107; spec 005 FR-002b). These performers are test data created by another extension's tests.
  The capture holds their tag ids, Archetype fields and measurements, but not their names.

**Files:**
- `library-read.json`: the whole `LibraryRead`. `MemoryReader::from_file` loads it.
- `tags.json`, `performers.json`, `scenes.json` and `images.json`: the same records split by
  kind.
- `index.json`: counts, empty sections, where the read started, ids that went missing, and the
  number of requests.

**Regenerate:**

```sh
STASH_URL=http://localhost:9998 STASH_API_KEY=... \
  cargo run -p stash-adapter --bin fixture-capture -- --out crates/stash-adapter/tests/fixtures/test-instance
```
