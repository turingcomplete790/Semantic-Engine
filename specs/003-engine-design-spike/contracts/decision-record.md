# Contract: `decision-record.md`

This is the spike's deliverable. It must read on its own (spec SC-006). Its structure is fixed so
that later specs can cite sections by name.

```markdown
# Decision Record: Engine Design Spike

Outcome: proceed | proceed-with-changes | fall-back
Date: YYYY-MM-DD · Stash: v0.31.1 · Engine commit: <sha> · Vocabulary: 1.1.0

## Summary
Three to six sentences: the outcome, and the one or two numbers that decided it.

## For client maintainers
- Should you depend on the engine: yes/no and when
- Page size for results: N bindings (or bytes)
- Expected freshness delay: N s after a save; catch-up after reconnection: how
- Cost the engine adds to every save: N ms per item (hooks are synchronous)

## Decisions
### 1. Process model
### 2. Persistence
### 3. Freshness
### 4. Result limits
### 5. Snapshot delivery
### 6. Build time and size
### 7. Go / no-go

## Rules for Phase 2 and Phase 3
Numbered, one sentence each (persistence, concurrency, freshness, paging, hook budget).

## Constitution amendments proposed
None, or principle + proposed text + reason.

## Estimates
Figures scaled beyond the measured sizes, each marked "estimate" with the per-item basis.
```

Each `### N.` section holds:
- **Decision:** one sentence.
- **Options:** a table with columns name, key figures, constitution, verdict.
- **Evidence:** Measurement ids, linked to `results/<id>.json`.
- **Reproduce:** the exact `spike-bench` commands.

Section 3 also includes the 12-row hook table, plus the bulk rows (data-model Hook observation).

**Acceptance**:
- All seven sections are present and none says "TBD" (SC-001).
- Every number links to a result file.
- The outcome line matches section 7.
