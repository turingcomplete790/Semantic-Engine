# Test-instance fixtures

Captured with `fixture-capture` (read-only queries) from the local Stash v0.31.1 test instance on
2026-10-16. Library counts were identical before and after the capture, and no credential is
recorded. The semantic data is the `SE-Test` items created for spec 001/002 testing plus three
`SSV Test` tags from the Semantic Stash Viewer's tests.

Regenerate:

```sh
STASH_URL=http://localhost:9998 STASH_API_KEY=... \
  cargo run -p stash-adapter --bin fixture-capture -- --out crates/stash-adapter/tests/fixtures/test-instance
```
