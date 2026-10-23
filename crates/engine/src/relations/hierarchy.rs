//! Role and relation hierarchies (spec 004 FR-002 to FR-004; research R4).
//!
//! A Stash parent is a broader term only when it has the same type: a Role under a Role, a
//! Relation under a Relation. Other parents keep their plain `parentTag` meaning. A tag may have
//! several broader terms. Every link that lies on a loop is ignored and the loop reported once;
//! a relation on a loop, or whose ends are not the same as or narrower than the ends of every
//! broader relation, is marked invalid.

use super::defs::{Definitions, Problem, ProblemKind};
use std::collections::{BTreeMap, BTreeSet};

type Graph = BTreeMap<String, BTreeSet<String>>;

#[derive(Debug, Clone, Default)]
pub struct Hierarchy {
    /// Direct broader terms, loops removed.
    up: Graph,
    /// Direct narrower terms, loops removed.
    down: Graph,
}

impl Hierarchy {
    /// Builds both hierarchies from `defs`, adds `loop` and `invalidEnds` problems to it, and
    /// marks the affected relations invalid.
    pub fn build(defs: &mut Definitions) -> Hierarchy {
        let mut up = Graph::new();
        for (id, t) in &defs.tags {
            let same_type = |p: &String| {
                (defs.is_role(id) && defs.is_role(p))
                    || (defs.is_relation(id) && defs.is_relation(p))
            };
            let parents: BTreeSet<String> =
                t.parents.iter().filter(|p| same_type(p)).cloned().collect();
            if !parents.is_empty() {
                up.insert(id.clone(), parents);
            }
        }

        for cycle in cycles(&up) {
            for t in &cycle {
                if let Some(ps) = up.get_mut(t) {
                    ps.retain(|p| !cycle.contains(p));
                }
                if let Some(r) = defs.relations.get_mut(t) {
                    r.valid = false;
                }
            }
            defs.problems.push(Problem {
                kind: ProblemKind::Loop,
                tags: cycle.iter().cloned().collect(),
                relation: None,
                detail: String::new(),
            });
        }
        up.retain(|_, ps| !ps.is_empty());

        let mut down = Graph::new();
        for (child, parents) in &up {
            for p in parents {
                down.entry(p.clone()).or_default().insert(child.clone());
            }
        }
        let h = Hierarchy { up, down };
        h.check_relation_ends(defs);
        h
    }

    /// Direct broader terms.
    pub fn broader(&self, tag: &str) -> BTreeSet<String> {
        self.up.get(tag).cloned().unwrap_or_default()
    }

    /// Every broader term at any depth, not including `tag` itself.
    pub fn broader_closure(&self, tag: &str) -> BTreeSet<String> {
        closure(&self.up, tag)
    }

    /// Narrower terms: direct children, or every depth when `recursive`.
    pub fn narrower(&self, tag: &str, recursive: bool) -> BTreeSet<String> {
        if recursive {
            closure(&self.down, tag)
        } else {
            self.down.get(tag).cloned().unwrap_or_default()
        }
    }

    /// The tag ids a search criterion for `tag` matches (spec 004 FR-013, US3): `tag` itself,
    /// and with `recursive` every narrower term at any depth. Phase 3's search operation calls
    /// this for Relation and Role criteria; `recursive` is off by default there.
    pub fn expand_criterion(&self, tag: &str, recursive: bool) -> BTreeSet<String> {
        let mut out = if recursive {
            self.narrower(tag, true)
        } else {
            BTreeSet::new()
        };
        out.insert(tag.to_string());
        out
    }

    /// True when `tag` is `ancestor` or narrower than it at any depth.
    pub fn is_same_or_narrower(&self, tag: &str, ancestor: &str) -> bool {
        tag == ancestor || self.broader_closure(tag).contains(ancestor)
    }

    fn check_relation_ends(&self, defs: &mut Definitions) {
        let mut bad = Vec::new();
        for (id, r) in &defs.relations {
            let Some((a, b)) = &r.ends else { continue };
            if !r.valid {
                continue;
            }
            for p in self.broader_closure(id) {
                let Some((pa, pb)) = defs.relations.get(&p).and_then(|pr| pr.ends.clone()) else {
                    continue;
                };
                for (name, end, parent_end) in [("a", a, &pa), ("b", b, &pb)] {
                    if !self.is_same_or_narrower(end, parent_end) {
                        bad.push((
                            id.clone(),
                            format!(
                                "end {name} {end} is not {parent_end} or narrower than it, as broader relation {p} requires"
                            ),
                        ));
                    }
                }
            }
        }
        for (id, detail) in bad {
            if let Some(r) = defs.relations.get_mut(&id) {
                r.valid = false;
            }
            defs.problems.push(Problem {
                kind: ProblemKind::InvalidEnds,
                tags: Vec::new(),
                relation: Some(id),
                detail,
            });
        }
    }
}

fn closure(g: &Graph, start: &str) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut stack: Vec<String> = g.get(start).into_iter().flatten().cloned().collect();
    while let Some(t) = stack.pop() {
        if t != start && seen.insert(t.clone()) {
            stack.extend(g.get(&t).into_iter().flatten().cloned());
        }
    }
    seen
}

/// The strongly connected components of `g` that contain a cycle (Tarjan), each as a sorted set.
fn cycles(g: &Graph) -> Vec<BTreeSet<String>> {
    struct State<'a> {
        g: &'a Graph,
        index: BTreeMap<&'a str, usize>,
        low: BTreeMap<&'a str, usize>,
        on_stack: BTreeSet<&'a str>,
        stack: Vec<&'a str>,
        next: usize,
        out: Vec<BTreeSet<String>>,
    }
    fn visit<'a>(s: &mut State<'a>, v: &'a str) {
        s.index.insert(v, s.next);
        s.low.insert(v, s.next);
        s.next += 1;
        s.stack.push(v);
        s.on_stack.insert(v);
        if let Some(ws) = s.g.get(v) {
            for w in ws {
                let w = w.as_str();
                if !s.index.contains_key(w) {
                    visit(s, w);
                    let lw = s.low[w];
                    let lv = s.low.get_mut(v).expect("visited");
                    *lv = (*lv).min(lw);
                } else if s.on_stack.contains(w) {
                    let iw = s.index[w];
                    let lv = s.low.get_mut(v).expect("visited");
                    *lv = (*lv).min(iw);
                }
            }
        }
        if s.low[v] == s.index[v] {
            let mut comp = BTreeSet::new();
            while let Some(w) = s.stack.pop() {
                s.on_stack.remove(w);
                comp.insert(w.to_string());
                if w == v {
                    break;
                }
            }
            let self_loop = s.g.get(v).is_some_and(|ws| ws.contains(v));
            if comp.len() > 1 || self_loop {
                s.out.push(comp);
            }
        }
    }
    let mut s = State {
        g,
        index: BTreeMap::new(),
        low: BTreeMap::new(),
        on_stack: BTreeSet::new(),
        stack: Vec::new(),
        next: 0,
        out: Vec::new(),
    };
    for v in g.keys() {
        if !s.index.contains_key(v.as_str()) {
            visit(&mut s, v.as_str());
        }
    }
    s.out
}
