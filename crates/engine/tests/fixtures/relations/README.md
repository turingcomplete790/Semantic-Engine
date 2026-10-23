# Relation fixtures (spec 004, task T002)

Offline fixtures shaped like `stash_adapter::relations` records. Custom fields that hold JSON
are JSON-encoded strings, as the semantic-tagging plugin stores them.

- `example1.json`: the owner's hierarchy diagram (Customer > Patient; Professional > Nurse,
  Doctor; Full Service > Extra Care). Nurse also has a non-Role parent (90), which must not count
  as a broader role.
- `example2.json`: the auto-apply diagram (Short + Tall paired on Kissing gives Size Difference,
  Smaller and Larger). Tag 22 is a common variant of Kissing.
- `invalid.json`: a role loop (70, 71), a relation missing an end (72), an end that is a Label
  (73), a narrower relation whose end is not narrower (74), and bad rules (75). It refers to tags
  in the two example files, so tests load all three together.
- `performers.json`: performers and their tag ids (Labels).
- `scenes.json`: one scene per case; each has a `note` saying what it covers.
