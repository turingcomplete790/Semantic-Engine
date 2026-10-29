//! The grammar of query language level 1 (spec 009 T005; contracts/query-language.md).

use engine::query::parse::{parse, CompareOp, IriRef, NodeKind, Projection};
use engine::query::{ErrorKind, Pos, QueryError};

fn ok(text: &str) -> engine::query::parse::Ast {
    parse(text).unwrap_or_else(|e| panic!("{e}\nquery:\n{text}"))
}

fn err(text: &str) -> QueryError {
    match parse(text) {
        Ok(a) => panic!("parsed, expected an error: {a:?}\nquery:\n{text}"),
        Err(e) => e,
    }
}

fn at(e: &QueryError) -> (u32, u32) {
    let p = e.pos.unwrap_or_else(|| panic!("no position: {e}"));
    (p.line, p.column)
}

fn unsupported(text: &str, construct: &str, pos: (u32, u32)) {
    let e = err(text);
    match &e.kind {
        ErrorKind::Unsupported { construct: c } => {
            assert_eq!(c, construct, "{e}\nquery:\n{text}");
        }
        _ => panic!("expected Unsupported({construct}), got {e:?}\nquery:\n{text}"),
    }
    assert_eq!(at(&e), pos, "{e}\nquery:\n{text}");
    assert_eq!(e.code().as_str(), "QUERY_UNSUPPORTED");
}

fn syntax(text: &str) -> QueryError {
    let e = err(text);
    assert_eq!(e.kind, ErrorKind::Syntax, "{e:?}\nquery:\n{text}");
    assert!(e.pos.is_some(), "no position: {e}");
    e
}

#[test]
fn the_documents_examples_parse() {
    let a =
        ok("SELECT ?scene WHERE {\n  ?scene :hasPerformer ?person .\n  ?person :hasRole :Dom .\n}");
    assert_eq!(a.variables(), vec!["scene"]);
    assert_eq!(a.patterns.len(), 2);
    let a = ok("SELECT ?scene ?dom ?sub WHERE {
    ?scene :hasPerformer ?dom .
    ?scene :hasPerformer ?sub .
    ?dom :hasRole :Dom .
    ?sub :hasRole :Sub .
    FILTER (?dom != ?sub)
}");
    assert_eq!(a.patterns.len(), 4);
    assert_eq!(a.filters.len(), 1);
    assert_eq!(a.filters[0].op, CompareOp::Ne);
}

#[test]
fn lists_prefixes_and_keywords_in_any_case() {
    let a = ok(
        "prefix ex: <http://example.org/>\nselect distinct * where { ?s a ex:T ; ex:p ?o , ?q . }",
    );
    assert!(a.distinct);
    assert_eq!(a.projection, Projection::All);
    assert_eq!(a.variables(), vec!["s", "o", "q"]);
    assert_eq!(a.patterns.len(), 3);
    assert_eq!(a.prefixes[0].0, "ex");
    match &a.patterns[0].p.kind {
        NodeKind::Iri(IriRef::Full(i)) => {
            assert_eq!(i, "http://www.w3.org/1999/02/22-rdf-syntax-ns#type")
        }
        k => panic!("{k:?}"),
    }
    // WHERE is optional, and the last '.' too.
    ok("SELECT ?s { ?s ?p ?o }");
    // A trailing ';'.
    ok("SELECT ?s { ?s :a ?o ; }");
}

#[test]
fn literals_and_names() {
    let a = ok(r#"SELECT ?s WHERE {
        ?s :p "plain" , "tagged"@en , "typed"^^xsd:decimal , 'single' , """long
text""" , 12 , -3 , 4.5 , 1e3 , true , FALSE , :Full%20Service , <urn:x:y> .
        # a comment
    }"#);
    let lits: Vec<String> = a
        .patterns
        .iter()
        .map(|t| match &t.o.kind {
            NodeKind::Literal {
                lexical,
                datatype,
                language,
            } => format!(
                "{lexical}|{}|{}",
                match datatype {
                    Some(IriRef::Full(d)) => d.rsplit('#').next().unwrap().to_string(),
                    Some(IriRef::Prefixed { prefix, local }) => format!("{prefix}:{local}"),
                    None => String::new(),
                },
                language.clone().unwrap_or_default()
            ),
            NodeKind::Iri(IriRef::Prefixed { prefix, local }) => format!("{prefix}:{local}"),
            NodeKind::Iri(IriRef::Full(i)) => format!("<{i}>"),
            k => panic!("{k:?}"),
        })
        .collect();
    assert_eq!(
        lits,
        vec![
            "plain||",
            "tagged||en",
            "typed|xsd:decimal|",
            "single||",
            "long\ntext||",
            "12|integer|",
            "-3|integer|",
            "4.5|decimal|",
            "1e3|double|",
            "true|boolean|",
            "false|boolean|",
            ":Full%20Service",
            "<urn:x:y>",
        ]
    );
}

#[test]
fn every_comparison_and_several_filters() {
    let a = ok("SELECT ?a WHERE { ?a :p ?x . ?a :q ?y .
        FILTER (?x = ?y) FILTER (?x != ?y) FILTER(?x < 3) FILTER(?x <= 3)
        FILTER(?x > 3.5) FILTER(?x >= \"a\") }");
    let ops: Vec<CompareOp> = a.filters.iter().map(|f| f.op).collect();
    use CompareOp::*;
    assert_eq!(ops, vec![Eq, Ne, Lt, Le, Gt, Ge]);
}

#[test]
fn every_node_has_its_position() {
    let a = ok("SELECT ?s\nWHERE {\n  ?s :p ?o .\n    ?o :q \"x\" .\n}");
    let p = |l, c| Pos { line: l, column: c };
    assert_eq!(a.patterns[0].s.pos, p(3, 3));
    assert_eq!(a.patterns[0].p.pos, p(3, 6));
    assert_eq!(a.patterns[0].o.pos, p(3, 9));
    assert_eq!(a.patterns[1].s.pos, p(4, 5));
    assert_eq!(a.patterns[1].o.pos, p(4, 11));
    assert_eq!(a.patterns[1].pos, p(4, 5));
}

#[test]
fn refused_constructs_are_named_at_their_position() {
    let w = "SELECT ?s WHERE { ?s ?p ?o ";
    unsupported("ASK { ?s ?p ?o }", "ASK", (1, 1));
    unsupported(
        "CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }",
        "CONSTRUCT",
        (1, 1),
    );
    unsupported("DESCRIBE ?s", "DESCRIBE", (1, 1));
    unsupported("INSERT DATA { <a:b> <a:c> <a:d> }", "an update", (1, 1));
    unsupported("BASE <http://x/> SELECT ?s { ?s ?p ?o }", "BASE", (1, 1));
    unsupported(
        &format!("{w}OPTIONAL {{ ?s ?q ?r }} }}"),
        "OPTIONAL",
        (1, 28),
    );
    unsupported(
        &format!("{w}. {{ ?a ?b ?c }} UNION {{ ?d ?e ?f }} }}"),
        "a nested group",
        (1, 30),
    );
    unsupported(&format!("{w}MINUS {{ ?s ?q ?r }} }}"), "MINUS", (1, 28));
    unsupported(&format!("{w}GRAPH ?g {{ ?s ?q ?r }} }}"), "GRAPH", (1, 28));
    unsupported(
        &format!("{w}SERVICE <x:y> {{ ?s ?q ?r }} }}"),
        "SERVICE",
        (1, 28),
    );
    unsupported(&format!("{w}BIND (1 AS ?x) }}"), "BIND", (1, 28));
    unsupported(&format!("{w}VALUES ?s {{ <a:b> }} }}"), "VALUES", (1, 28));
    unsupported(
        &format!("{w}. {{ ?a ?b ?c }} }}"),
        "a nested group",
        (1, 30),
    );
    unsupported(
        &format!("{w}. {{ SELECT ?a {{ ?a ?b ?c }} }} }}"),
        "a nested group",
        (1, 30),
    );
    unsupported("SELECT ?s WHERE { ?s :a/:b ?o }", "property paths", (1, 24));
    unsupported("SELECT ?s WHERE { ?s :a|:b ?o }", "property paths", (1, 24));
    unsupported("SELECT ?s WHERE { ?s ^:a ?o }", "property paths", (1, 22));
    unsupported("SELECT ?s WHERE { ?s :a* ?o }", "property paths", (1, 24));
    unsupported("SELECT ?s WHERE { ?s :a+ ?o }", "property paths", (1, 24));
    unsupported("SELECT ?s WHERE { ?s :a? ?o }", "property paths", (1, 24));
    unsupported("SELECT ?s WHERE { ?s :a _:b }", "blank nodes", (1, 25));
    unsupported("SELECT ?s WHERE { [] :a ?s }", "blank nodes", (1, 19));
    unsupported("SELECT ?s WHERE { ?s :a ( 1 2 ) }", "collections", (1, 25));
    unsupported(&format!("{w}FILTER (?s = ?o && ?p = ?o) }}"), "&&", (1, 44));
    unsupported(&format!("{w}FILTER (?s = ?o || ?p = ?o) }}"), "||", (1, 44));
    unsupported(&format!("{w}FILTER (!?s) }}"), "!", (1, 36));
    unsupported(
        &format!("{w}FILTER (?s + 1 = ?o) }}"),
        "arithmetic",
        (1, 39),
    );
    unsupported(
        &format!("{w}FILTER regex(?s, \"x\") }}"),
        "the function regex",
        (1, 35),
    );
    unsupported(
        &format!("{w}FILTER (STR(?s) = \"x\") }}"),
        "the function STR",
        (1, 36),
    );
    unsupported(
        &format!("{w}FILTER NOT EXISTS {{ ?s ?q ?r }} }}"),
        "EXISTS",
        (1, 35),
    );
    unsupported(&format!("{w}}} GROUP BY ?s"), "GROUP BY", (1, 30));
    unsupported(&format!("{w}}} HAVING (?s)"), "HAVING", (1, 30));
    unsupported(&format!("{w}}} ORDER BY ?s"), "ORDER BY", (1, 30));
    unsupported(&format!("{w}}} LIMIT 5"), "LIMIT", (1, 30));
    unsupported(&format!("{w}}} OFFSET 5"), "OFFSET", (1, 30));
    unsupported(
        "SELECT (COUNT(?s) AS ?n) WHERE { ?s ?p ?o }",
        "expressions in SELECT",
        (1, 8),
    );
    unsupported("SELECT ?s FROM <x:g> WHERE { ?s ?p ?o }", "FROM", (1, 11));
}

#[test]
fn syntax_errors_have_positions() {
    let e = syntax("SELECT ?s WHERE { ?s ?p ?o");
    assert!(e.message.contains("'}'"), "{e}");
    let e = syntax("SELECT ?s WHERE {\n  ?s ?p ?o\n  ?a ?b ?c }");
    assert_eq!(at(&e), (3, 3));
    let e = syntax("SELECT ?s WHERE { ?s ?p \"open }");
    assert_eq!(at(&e), (1, 25));
    let e = syntax("SELECT ?x WHERE { ?s ?p ?o }");
    assert_eq!(at(&e), (1, 8));
    assert!(e.message.contains("?x"), "{e}");
    let e = syntax("SELECT ?s WHERE { ?s ?p ?__se1 }");
    assert_eq!(at(&e), (1, 25));
    let e = syntax("SELECT ?__se2 WHERE { ?s ?p ?__se2 }");
    assert_eq!(at(&e), (1, 8));
    let e = syntax("SELECT ?s WHERE { ?s ?p ?o FILTER (?z = 1) }");
    assert_eq!(at(&e), (1, 36));
    syntax("SELECT ?s WHERE { }");
    syntax("SELECT WHERE { ?s ?p ?o }");
    syntax("SELECT ?s WHERE { \"lit\" ?p ?o }");
}

#[test]
fn built_in_prefixes_cannot_be_redefined() {
    for p in [
        "sem",
        "",
        "tag",
        "performer",
        "scene",
        "image",
        "recursive",
        "recursive-tag",
        "rdf",
        "rdfs",
        "xsd",
    ] {
        let e = syntax(&format!(
            "PREFIX {p}: <http://example.org/>\nSELECT ?s {{ ?s ?p ?o }}"
        ));
        assert_eq!(at(&e), (1, 1), "{p}");
        assert!(e.message.contains("built in"), "{e}");
    }
}
