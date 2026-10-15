# Contract: Vocabulary Files

Version of this contract: 1.0.0

## Files

| File | Content |
|---|---|
| `vocabulary/core.ttl` | Classes and properties for media, performers, tags, assignments, pairings, variations, archetypes |
| `vocabulary/labels.ttl` | Label formula classes and properties |
| `vocabulary/timeline.ttl` | Timeline classes and properties |
| `vocabulary/stash-mapping.ttl` | Mapping from Stash data to terms |
| `vocabulary/CHANGELOG.md` | Version history and migration notes |
| `vocabulary/TERMS.md` | Generated term list; never edited by hand |
| `vocabulary/released/<version>.json` | Snapshot of each released version's terms with domain and range; only added at release, never edited |

All `.ttl` files are Turtle 1.1, UTF-8, LF line endings, and declare the same prefixes:

```turtle
@prefix sem:  <urn:semantic-stash:vocab:> .
@prefix rdf:  <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix xsd:  <http://www.w3.org/2001/XMLSchema#> .
```

The vocabulary version is declared once, in `core.ttl`, on `<urn:semantic-stash:vocab:>`
(`sem:version "1.0.0"`). Other files declare no version; the check enforces this.

## Required annotations on every term

| Annotation | Value | Rule |
|---|---|---|
| `rdfs:label` | string, language-tagged `@en` | Exactly one |
| `rdfs:comment` | string | Exactly one; a plain-language definition |
| `sem:source` | string: `document § section` | At least one; documents named as in the spec |
| `sem:sourceRevision` | string: plugin commit or date | Exactly one |
| `sem:scope` | `sem:SceneScoped` or `sem:Global` | Exactly one on every property and on every assignment class |
| `sem:status` | `sem:Active`, `sem:Deprecated`, `sem:Planned`, `sem:Derived` | Exactly one |
| `sem:since` | string version | Exactly one |

`Scene`, `Image`, `Performer`, `Tag`, and the Stash marker reference add `sem:references`
naming the Stash entity they stand for (a string such as `Scene`), so the vocabulary refers to
Stash entities rather than redefining them (FR-009).

Deprecated terms add `sem:replacedBy` naming the replacement term (or `sem:retiredNote`
when there is none). Planned terms add `rdfs:comment` text beginning "Planned:".

The vocabulary's own annotation and structural terms (`sem:source`, `sem:scope`, `sem:status`,
`sem:Mapping`, and the rest) are defined in `core.ttl` under the same rules as semantic terms.

## Structure rules

- RDFS only: `rdfs:Class`, `rdf:Property`, `rdfs:subClassOf`, `rdfs:subPropertyOf`,
  `rdfs:domain`, `rdfs:range`. No `owl:` terms.
- Every property has exactly one `rdfs:domain` and one `rdfs:range`. A union is expressed with
  a shared superclass, never with several `rdfs:domain` values.
- Names: classes in `UpperCamelCase`, properties in `lowerCamelCase`, all in `sem:`.
- A term is defined in exactly one file.

## Mapping entries

```turtle
sem:map_isRole a sem:Mapping ;
    sem:fromEntity "Tag" ;
    sem:fromField  "custom_fields.isRole" ;
    sem:allowedValues "1" ;
    sem:produces sem:Role ;
    sem:source "README.md § Features (tagging dimensions)" ;
    sem:sourceRevision "3c82c85+wc 2026-10-14" ;
    sem:status sem:Active ;
    sem:since "1.0.0" .
```

A mapping entry whose Stash location is not yet fixed uses `sem:status sem:Planned` and a
`rdfs:comment` beginning "Planned:"; the server identifier entry is the first of these.

## Compatibility

MAJOR: removing or renaming an active term, changing a domain or range incompatibly, or
changing the item IRI scheme. MINOR: adding terms or mappings, marking a term deprecated.
PATCH: wording, annotations, and corrections that do not change graph meaning.
