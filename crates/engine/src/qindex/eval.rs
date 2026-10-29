//! Evaluating a rewritten level-1 plan on the query index (spec 009 research R12). Patterns are
//! joined depth-first by index range lookups, in an order chosen once, by the expected fan-out of
//! each pattern given what is already bound (from the index's predicate statistics). Each filter
//! runs as soon as its variables are bound. The answer is the set of projected bindings in
//! evaluation order, which is the same on every evaluation of one generation (FR-008); it is not
//! sorted, so evaluation can stop as soon as a page is full.

use super::{spo, Order, QIndex};
use crate::graph::Term;
use crate::query::parse::CompareOp;
use crate::query::rewrite::{PTerm, Plan};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

const UNBOUND: u32 = u32::MAX;
/// An object block this small is scanned rather than searched by predicate.
const SMALL_BLOCK: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Slot {
    Var(usize),
    Const(u32),
}

/// A filter operand: a variable, a constant held by the index, or a constant it does not hold.
#[derive(Debug, Clone)]
enum Operand {
    Var(usize),
    Id(u32),
    Absent(Term),
}

enum Step {
    Pattern([Slot; 3]),
    /// A `VALUES` variable: enumerated when unbound, checked when bound.
    Values(usize, Vec<u32>),
}

struct Planned {
    steps: Vec<Step>,
    /// Filters to check after each step (index = step).
    filters: Vec<Vec<(Operand, CompareOp, Operand)>>,
    projected: Vec<usize>,
    slots: usize,
}

/// The whole answer: every distinct binding of the projected variables, as term ids.
pub fn evaluate(index: &QIndex, plan: &Plan) -> Vec<Vec<u32>> {
    stream(index, plan, usize::MAX)
}

/// The first `want` distinct bindings in evaluation order, which is deterministic for one
/// generation and one plan; evaluation stops as soon as it has them.
pub fn stream(index: &QIndex, plan: &Plan, want: usize) -> Vec<Vec<u32>> {
    let mut rows = Vec::new();
    each(index, plan, &mut |row| {
        rows.push(row.to_vec());
        rows.len() < want
    });
    rows
}

/// Calls `accept` with each distinct binding, in evaluation order, until it returns false.
pub fn each(index: &QIndex, plan: &Plan, accept: &mut dyn FnMut(&[u32]) -> bool) {
    let Some(planned) = prepare(index, plan) else {
        return;
    };
    let mut binding = vec![UNBOUND; planned.slots];
    let mut sink = Sink {
        seen: HashSet::new(),
        accept,
        stopped: false,
    };
    let mut terms = HashMap::new();
    run(index, &planned, 0, &mut binding, &mut sink, &mut terms);
}

struct Sink<'a> {
    seen: HashSet<Vec<u32>>,
    accept: &'a mut dyn FnMut(&[u32]) -> bool,
    stopped: bool,
}

impl Sink<'_> {
    fn full(&self) -> bool {
        self.stopped
    }
}

fn prepare(index: &QIndex, plan: &Plan) -> Option<Planned> {
    let mut names: HashMap<String, usize> = HashMap::new();
    let mut var = |n: &str| {
        let next = names.len();
        *names.entry(n.to_string()).or_insert(next)
    };
    let mut patterns = Vec::new();
    for [s, p, o] in &plan.patterns {
        let mut slot = |t: &PTerm| match t {
            PTerm::Var(v) => Some(Slot::Var(var(v))),
            PTerm::Term(t) => index.id(t).map(Slot::Const),
        };
        patterns.push([slot(s)?, slot(p)?, slot(o)?]);
    }
    let values: Vec<(usize, Vec<u32>)> = plan
        .values
        .iter()
        .map(|(v, ts)| (var(v), ts.iter().filter_map(|t| index.id(t)).collect()))
        .collect();
    let filters: Vec<(Operand, CompareOp, Operand)> = plan
        .filters
        .iter()
        .map(|(l, op, r)| {
            let mut operand = |t: &PTerm| match t {
                PTerm::Var(v) => Operand::Var(var(v)),
                PTerm::Term(t) => index.id(t).map_or(Operand::Absent(t.clone()), Operand::Id),
            };
            (operand(l), *op, operand(r))
        })
        .collect();
    let projected: Vec<usize> = plan.variables.iter().map(|v| var(v)).collect();
    let slots = names.len();

    // The order of steps.
    let mut bound = vec![false; slots];
    let mut steps = Vec::new();
    let mut left: Vec<[Slot; 3]> = patterns;
    let mut values_left = values;
    // The expected number of matches of a pattern, given which variables are bound: exact for
    // constants alone, and the predicate's average fan-out for each bound variable.
    let estimate = |p: &[Slot; 3], bound: &[bool]| -> f64 {
        let is_bound = |s: &Slot| match s {
            Slot::Const(_) => true,
            Slot::Var(v) => bound[*v],
        };
        let var_bound = |s: &Slot| matches!(s, Slot::Var(v) if bound[*v]);
        let (sb, ob) = (is_bound(&p[0]), is_bound(&p[2]));
        if !var_bound(&p[0]) && !var_bound(&p[1]) && !var_bound(&p[2]) {
            let (order, prefix) = lookup(&[const_of(p[0]), const_of(p[1]), const_of(p[2])]);
            return index.range(order, &prefix).len() as f64;
        }
        match p[1] {
            Slot::Const(pred) => {
                let [facts, subjects, objects] = index.stats(pred).unwrap_or([0, 1, 1]);
                let facts = facts as f64;
                match (sb, ob) {
                    (true, true) => 1.0_f64.min(facts),
                    (true, false) => facts / subjects.max(1) as f64,
                    (false, true) => facts / objects.max(1) as f64,
                    (false, false) => facts,
                }
            }
            Slot::Var(_) => match (sb, ob) {
                (true, true) => 2.0,
                (true, false) | (false, true) => 20.0,
                (false, false) => index.facts() as f64,
            },
        }
    };
    while !left.is_empty() || !values_left.is_empty() {
        let bound_count = |p: &[Slot; 3]| {
            p.iter()
                .filter(|s| match s {
                    Slot::Const(_) => true,
                    Slot::Var(v) => bound[*v],
                })
                .count()
        };
        // VALUES of a variable already bound is a check; place it now.
        if let Some(i) = values_left.iter().position(|(v, _)| bound[*v]) {
            let (v, ids) = values_left.remove(i);
            steps.push(Step::Values(v, ids));
            continue;
        }
        // The cheapest next pattern; among equals, the one with more positions bound.
        let best_pattern = left
            .iter()
            .enumerate()
            .map(|(i, p)| (i, bound_count(p), estimate(p, &bound)))
            .min_by(|a, b| a.2.total_cmp(&b.2).then(b.1.cmp(&a.1)));
        let best_values = values_left
            .iter()
            .enumerate()
            .min_by_key(|(_, (_, ids))| ids.len());
        let take_values = match (best_pattern, best_values) {
            (None, Some(_)) => true,
            (Some((_, _, cost)), Some((_, (_, ids)))) => (ids.len() as f64) < cost,
            _ => false,
        };
        if take_values {
            let i = best_values.expect("some").0;
            let (v, ids) = values_left.remove(i);
            bound[v] = true;
            steps.push(Step::Values(v, ids));
        } else {
            let (i, _, _) = best_pattern.expect("some");
            let p = left.remove(i);
            for s in p {
                if let Slot::Var(v) = s {
                    bound[v] = true;
                }
            }
            steps.push(Step::Pattern(p));
        }
    }

    // Each filter after the first step that binds all its variables.
    let mut at = vec![Vec::new(); steps.len().max(1)];
    let mut seen = vec![false; slots];
    let mut placed = vec![false; filters.len()];
    for (i, step) in steps.iter().enumerate() {
        match step {
            Step::Pattern(p) => {
                for s in p {
                    if let Slot::Var(v) = s {
                        seen[*v] = true;
                    }
                }
            }
            Step::Values(v, _) => seen[*v] = true,
        }
        for (f, filter) in filters.iter().enumerate() {
            let ready = [&filter.0, &filter.2].iter().all(|o| match o {
                Operand::Var(v) => seen[*v],
                _ => true,
            });
            if !placed[f] && ready {
                at[i].push(filter.clone());
                placed[f] = true;
            }
        }
    }
    Some(Planned {
        steps,
        filters: at,
        projected,
        slots,
    })
}

fn const_of(s: Slot) -> Option<u32> {
    match s {
        Slot::Const(c) => Some(c),
        Slot::Var(_) => None,
    }
}

/// The order and prefix that serve a pattern with these bound positions.
fn lookup(b: &[Option<u32>; 3]) -> (Order, Vec<u32>) {
    match *b {
        [Some(s), Some(p), Some(o)] => (Order::Spo, vec![s, p, o]),
        [Some(s), Some(p), None] => (Order::Spo, vec![s, p]),
        [Some(s), None, Some(o)] => (Order::Osp, vec![o, s]),
        [None, Some(p), Some(o)] => (Order::Pos, vec![p, o]),
        [Some(s), None, None] => (Order::Spo, vec![s]),
        [None, Some(p), None] => (Order::Pos, vec![p]),
        [None, None, Some(o)] => (Order::Osp, vec![o]),
        [None, None, None] => (Order::Spo, vec![]),
    }
}

fn run(
    index: &QIndex,
    plan: &Planned,
    step: usize,
    binding: &mut Vec<u32>,
    out: &mut Sink<'_>,
    terms: &mut HashMap<u32, Term>,
) {
    if out.full() {
        return;
    }
    if step == plan.steps.len() {
        let row: Vec<u32> = plan.projected.iter().map(|v| binding[*v]).collect();
        if !out.seen.contains(&row) {
            out.stopped = !(out.accept)(&row);
            out.seen.insert(row);
        }
        return;
    }
    let mut next = |binding: &mut Vec<u32>, terms: &mut HashMap<u32, Term>| {
        if plan.filters[step]
            .iter()
            .all(|f| holds(index, f, binding, terms))
        {
            run(index, plan, step + 1, binding, out, terms);
        }
    };
    match &plan.steps[step] {
        Step::Values(v, ids) => {
            if binding[*v] != UNBOUND {
                if ids.contains(&binding[*v]) {
                    next(binding, terms);
                }
            } else {
                for id in ids {
                    binding[*v] = *id;
                    next(binding, terms);
                }
                binding[*v] = UNBOUND;
            }
        }
        Step::Pattern(p) => {
            let value = |s: Slot, binding: &Vec<u32>| match s {
                Slot::Const(c) => Some(c),
                Slot::Var(v) => (binding[v] != UNBOUND).then_some(binding[v]),
            };
            let b = [
                value(p[0], binding),
                value(p[1], binding),
                value(p[2], binding),
            ];
            // With the object bound, its own block is often far smaller than the predicate's.
            let (order, prefix) = match b {
                [None, Some(_), Some(o)] if index.block(Order::Osp, o) <= SMALL_BLOCK => {
                    (Order::Osp, vec![o])
                }
                _ => lookup(&b),
            };
            for i in index.range(order, &prefix) {
                let fact = spo(order, index.entry(order, i));
                let mut newly = [usize::MAX; 3];
                let mut ok = true;
                for k in 0..3 {
                    match p[k] {
                        Slot::Const(c) => ok &= fact[k] == c,
                        Slot::Var(v) => {
                            if binding[v] == UNBOUND {
                                binding[v] = fact[k];
                                newly[k] = v;
                            } else if binding[v] != fact[k] {
                                ok = false;
                            }
                        }
                    }
                }
                if ok {
                    next(binding, terms);
                }
                for v in newly {
                    if v != usize::MAX {
                        binding[v] = UNBOUND;
                    }
                }
            }
        }
    }
}

fn holds(
    index: &QIndex,
    (l, op, r): &(Operand, CompareOp, Operand),
    binding: &[u32],
    terms: &mut HashMap<u32, Term>,
) -> bool {
    // `=` and `!=` between two held terms, when one is an IRI or a blank node: equal ids are the
    // same term, and such a term equals only itself, so nothing needs decoding.
    let id_of = |o: &Operand| match o {
        Operand::Var(v) => Some(binding[*v]),
        Operand::Id(id) => Some(*id),
        Operand::Absent(_) => None,
    };
    if let (Some(x), Some(y), CompareOp::Eq | CompareOp::Ne) = (id_of(l), id_of(r), op) {
        if x == y || !index.is_literal(x) || !index.is_literal(y) {
            return (x == y) == (*op == CompareOp::Eq);
        }
    }
    let mut term = |o: &Operand| -> Term {
        match o {
            Operand::Var(v) => {
                let id = binding[*v];
                terms.entry(id).or_insert_with(|| index.term(id)).clone()
            }
            Operand::Id(id) => terms.entry(*id).or_insert_with(|| index.term(*id)).clone(),
            Operand::Absent(t) => t.clone(),
        }
    };
    let (a, b) = (term(l), term(r));
    match compare(&a, &b) {
        Some(ord) => match op {
            CompareOp::Eq => ord == Ordering::Equal,
            CompareOp::Ne => ord != Ordering::Equal,
            CompareOp::Lt => ord == Ordering::Less,
            CompareOp::Le => ord != Ordering::Greater,
            CompareOp::Gt => ord == Ordering::Greater,
            CompareOp::Ge => ord != Ordering::Less,
        },
        // Terms of kinds that do not compare: equal only when identical, never ordered.
        None => match op {
            CompareOp::Eq => a == b,
            CompareOp::Ne => a != b,
            _ => false,
        },
    }
}

/// SPARQL's operator comparison for the cases level 1 meets: numbers by value, strings and
/// other literals of one datatype by lexical form, IRIs only by identity.
fn compare(a: &Term, b: &Term) -> Option<Ordering> {
    use crate::query::page::numeric;
    match (a, b) {
        (
            Term::Literal {
                value: v1,
                datatype: d1,
                language: l1,
            },
            Term::Literal {
                value: v2,
                datatype: d2,
                language: l2,
            },
        ) => {
            if let (Some(x), Some(y)) = (numeric(d1.as_deref(), v1), numeric(d2.as_deref(), v2)) {
                return x.partial_cmp(&y);
            }
            (d1 == d2 && l1 == l2).then(|| v1.cmp(v2))
        }
        _ => None,
    }
}

/// The chosen steps, for profiling: each pattern with its constant-only range size.
pub fn explain(index: &QIndex, plan: &Plan) -> Vec<String> {
    let Some(planned) = prepare(index, plan) else {
        return vec!["a constant is not in the generation: no answer".into()];
    };
    planned
        .steps
        .iter()
        .enumerate()
        .map(|(i, s)| match s {
            Step::Pattern(p) => {
                let (order, prefix) = lookup(&[const_of(p[0]), const_of(p[1]), const_of(p[2])]);
                format!(
                    "{i}: pattern {:?} (constants alone: {} entries); filters {}",
                    p,
                    index.range(order, &prefix).len(),
                    planned.filters[i].len()
                )
            }
            Step::Values(v, ids) => format!("{i}: values ?{v} ({} ids)", ids.len()),
        })
        .collect()
}
