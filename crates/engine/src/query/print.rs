//! Printing a plan as the SPARQL text the store evaluates (spec 009 research R1, R4). The text
//! is the engine's own: it may use `VALUES`, which clients cannot write. It has no `ORDER BY`,
//! `OFFSET` or `LIMIT`: the engine orders and slices the answer itself (research R4), which
//! Oxigraph's own sort made about twenty times slower.

use super::rewrite::{PTerm, Plan};
use std::fmt::Write;

fn term(t: &PTerm) -> String {
    match t {
        PTerm::Var(v) => format!("?{v}"),
        PTerm::Term(t) => t.to_ntriples(),
    }
}

/// `SELECT DISTINCT ?v… WHERE { … }`.
pub fn print(plan: &Plan) -> String {
    let vars: Vec<String> = plan.variables.iter().map(|v| format!("?{v}")).collect();
    let vars = vars.join(" ");
    let mut q = format!("SELECT DISTINCT {vars} WHERE {{\n");
    for (v, tags) in &plan.values {
        let list: Vec<String> = tags.iter().map(|t| t.to_ntriples()).collect();
        let _ = writeln!(q, "  VALUES ?{v} {{ {} }}", list.join(" "));
    }
    for [s, p, o] in &plan.patterns {
        let _ = writeln!(q, "  {} {} {} .", term(s), term(p), term(o));
    }
    for (l, op, r) in &plan.filters {
        let _ = writeln!(q, "  FILTER ({} {} {})", term(l), op.as_str(), term(r));
    }
    q.push('}');
    q
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Term;
    use crate::query::parse::CompareOp;

    #[test]
    fn prints_values_and_filters() {
        let v = |s: &str| PTerm::Var(s.into());
        let plan = Plan {
            variables: vec!["s".into(), "t".into()],
            patterns: vec![[v("s"), PTerm::Term(Term::iri("urn:x:p")), v("__se1")]],
            values: vec![(
                "__se1".into(),
                vec![Term::iri("urn:x:a"), Term::iri("urn:x:b")],
            )],
            filters: vec![(v("s"), CompareOp::Ne, PTerm::Term(Term::string("a \"b\"")))],
        };
        assert_eq!(
            print(&plan),
            "SELECT DISTINCT ?s ?t WHERE {\n  VALUES ?__se1 { <urn:x:a> <urn:x:b> }\n  \
             ?s <urn:x:p> ?__se1 .\n  FILTER (?s != \"a \\\"b\\\"\")\n}"
        );
    }
}
