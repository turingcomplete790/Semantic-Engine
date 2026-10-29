# The query language, level 1

The language the [`query`](api.md#query) operation accepts in contract 1.4.0. This is a copy of
[`specs/009-query-operations/contracts/query-language.md`](../specs/009-query-operations/contracts/query-language.md);
the two change together.

It is the semantic-tagging documents' Phase 1
query language, plus single comparisons (§ Query Features, Phase 1 and Phase 2). Everything
accepted here stays accepted in every 1.x contract (Principle VI). Later levels add to it; the
roadmap's Phase 5 holds boolean logic and further constructs.

## Grammar

Keywords are case-insensitive. Comments start with `#` and run to the end of the line.

```text
Query        := Prologue Select
Prologue     := ( "PREFIX" PNAME_NS IRIREF )*
Select       := "SELECT" "DISTINCT"? ( Var+ | "*" ) "WHERE"? Group
Group        := "{" ( Triples | Filter )* "}"
Triples      := Subject PropertyList "."?
PropertyList := Verb ObjectList ( ";" ( Verb ObjectList )? )*
ObjectList   := Object ( "," Object )*
Subject      := Var | Iri
Verb         := Var | Iri | "a"
Object       := Var | Iri | Literal
Filter       := "FILTER" "(" Operand Op Operand ")"
Operand      := Var | Iri | Literal
Op           := "=" | "!=" | "<" | "<=" | ">" | ">="
Iri          := IRIREF | PrefixedName
Literal      := String ( "@" LANG | "^^" Iri )? | Integer | Decimal | Double | "true" | "false"
```

`IRIREF`, `PrefixedName`, `PNAME_NS`, `String`, the numbers, `LANG` and `Var` are SPARQL 1.1's
terminals. Local names may use percent encoding (`:Full%20Service`).

- **Several `FILTER`s** all apply.
- **`SELECT *`** projects every variable the query names, in order of first appearance.
- **A projected variable** must appear in a triple pattern.
- **Variables starting `__se`** are reserved for the engine.

## Refused constructs

Each is refused before evaluation with `QUERY_UNSUPPORTED`, naming it and its position:
- **other query forms and updates:** `ASK`, `CONSTRUCT`, `DESCRIBE`, any update;
- **`BASE`**;
- **graph patterns:** `OPTIONAL`, `UNION`, `MINUS`, `GRAPH`, `SERVICE`, `BIND`, `VALUES`,
  nested `{ }` groups, sub-queries;
- **property paths** (`/`, `|`, `^`, `*`, `+`, `?` in a verb);
- **blank nodes** (`_:b`, `[ ]`) and collections (`( )`);
- **in `FILTER`:** `&&`, `||`, `!`, arithmetic, and functions;
- **solution modifiers in the text:** `GROUP BY`, `HAVING`, `ORDER BY`, `LIMIT`, `OFFSET`, and
  aggregates. Page size and limit are arguments (operations contract).
- **`FROM`**, `FROM NAMED`.

## Names

Built-in prefixes are always present. A query's `PREFIX` lines may add others, but may not
redefine a built-in one: that is `QUERY_SYNTAX`.

| Prefix | Expands to | Checked |
|---|---|---|
| `sem:` | `urn:semantic-stash:vocab:` | an Active term of the vocabulary |
| `:` | a vocabulary term if the local name is one, otherwise a tag by its exact Stash name | the term, or exactly one tag with that name |
| `tag:`, `performer:`, `scene:`, `image:` | `urn:semantic-stash:{serverId}:{kind}:` + id | the item exists in the generation |
| `recursive:` | a tag by its exact Stash name, marked recursive | exactly one tag, which is a Role or a Relation |
| `recursive-tag:` | a tag by id, marked recursive | the tag exists and is a Role or a Relation |
| `rdf:`, `rdfs:`, `xsd:` | the W3C namespaces | not checked |

- **A term that is not Active** is refused, with its status in the message. For example,
  `sem:hasTag` is Planned, and `sem:inStage` is Derived and not stored. A Derived term that the
  builder stores (one a Stash mapping produces, such as `sem:bustSize`) is accepted.
- **Full IRIs** in the vocabulary or item namespaces are checked like their prefixed forms.
  Other IRIs are taken as written.

## Meaning

### Short form and scope

A triple whose verb is `hasRole`, `hasAction`, `hasPluggable` or `hasMiseEnScene`, and whose
object is a tag, is short form.
- **A tag object** is a tag constant, or a variable that is never the subject of another triple
  in the query.
- **Any other triple** is in the graph's own shape, and is matched as written.

| Short form | Matches as |
|---|---|
| `?p :hasRole X` with scope `M` | `?p sem:hasRole ?i . ?i sem:ofTag X . ?i sem:inMedia M` |
| `?p :hasAction X`, `?p :hasPluggable X` | the same, with `hasAction` or `hasPluggable` |
| `?m :hasMiseEnScene X` | `?m sem:hasMiseEnScene ?i . ?i sem:ofTag X` |

**The scope `M`** of a performer `?p` is the media term in the query's `M :hasPerformer ?p`:
- exactly one such `M`: the fact is tied to it;
- none: some media entity;
- more than one: refused with `QUERY_SCOPE_AMBIGUOUS` at the short-form triple.

**Global facts** are matched as written: `hasTrait`, `hasLabel`, tag categories and variations,
`parentTag`, `broaderRole`, `broaderRelation`, and the other `Global`-scope terms of
`vocabulary/TERMS.md`.

### Recursive criteria

A `recursive:` or `recursive-tag:` constant matches the tag and every narrower Role or Relation
at any depth, by the hierarchy of spec 004: same-type Stash parents, with links on a loop
ignored. Without the mark, a constant matches only itself.

### Answers

- **Set semantics:** each distinct binding of the projected variables appears once.
- **Order:** deterministic but unspecified. The same query on the same generation gives the same
  bindings in the same order, so pages never miss or repeat a binding, but the order is not
  sorted: it is the order the engine finds them in, which lets a page be returned as soon as it
  is full. Sort on the client when an order matters.
- **`FILTER`:** comparisons follow SPARQL 1.1. A comparison whose operands cannot be compared
  (for example a number and an IRI with `<`) is false for that binding, not an error.

## Examples

The documents' examples, as accepted:

```sparql
SELECT ?scene WHERE {
  ?scene :hasPerformer ?person .
  ?person :hasRole :Dom .
}
```

```sparql
SELECT ?scene ?dom ?sub WHERE {
  ?scene :hasPerformer ?dom .
  ?scene :hasPerformer ?sub .
  ?dom :hasRole :Dom .
  ?sub :hasRole :Sub .
  FILTER (?dom != ?sub)
}
```

Graph form, with a recursive relation and a time comparison:

```sparql
SELECT ?scene ?a ?b WHERE {
  ?r sem:ofRelation recursive:Full%20Service ;
     sem:inMedia ?scene ;
     sem:atEndA ?a ;
     sem:atEndB ?b .
  ?a sem:hasAction ?x . ?x sem:inMedia ?scene ; sem:atTime ?t1 .
  ?b sem:hasAction ?y . ?y sem:inMedia ?scene ; sem:atTime ?t2 .
  FILTER (?t1 < ?t2)
}
```
