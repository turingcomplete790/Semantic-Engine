//! The query index (spec 009 research R12): one read-only file per published generation,
//! `snapshots/<n>.qindex`, holding every fact of the generation as term ids, in three sorted
//! orders, with a dictionary of the terms. The engine evaluates level-1 queries on it itself
//! ([`eval`]), because Oxigraph's evaluation costs 40–640 µs per lookup at 250,000 scenes.
//!
//! ```text
//! header   magic, term count, entry count, section offsets
//! terms    offsets [u64; n+1], then the encoded terms in id order
//! lookup   [u32; n]: the ids sorted by encoded term, for finding a term's id
//! spo      [[u32; 4]; m]: (s, p, o, owner), sorted
//! pos      [[u32; 4]; m]: (p, o, s, owner), sorted
//! osp      [[u32; 4]; m]: (o, s, p, owner), sorted
//! starts   for each order, [u32; n+1]: where the entries with each first id begin
//! stats    k, then [p, entries, distinct subjects, distinct objects]: [u32; 4] per predicate
//! ```
//!
//! Each entry carries the owner graph of its fact (spec 008 R1), so that an update can drop
//! exactly the facts of the owners it touched and merge in their new ones ([`merge`]). Queries
//! ignore owners: they answer over the union, as Oxigraph's default graph does. The dictionary
//! only grows within a line of updates; a full build starts it again.

pub mod eval;

use crate::graph::Term;
use std::collections::{BTreeSet, HashMap};

/// What the index of a writer's next generation is made from.
pub(crate) enum Plan {
    /// Every fact, collected as the writer adds them to an empty store.
    Collect(Builder),
    /// The base generation's index, with the facts of these owners (`None`: the default graph)
    /// replaced by their current ones.
    Delta(BTreeSet<Option<String>>),
    /// The writer cannot follow its changes: read every fact of the store at publish.
    Rescan,
}
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 8] = b"SEQI0004";
const SECTIONS: usize = 10;
const HEADER: usize = 8 + 4 + 8 + 8 * SECTIONS;
/// The owner of facts in the default graph (none in store format 2).
pub const NO_OWNER: u32 = u32::MAX;

/// The index file of generation `n` in the store folder `dir`.
pub fn path(dir: &Path, n: u64) -> PathBuf {
    dir.join("snapshots").join(format!("{n}.qindex"))
}

/// A term's bytes in the dictionary: a kind byte, then length-prefixed parts.
pub fn encode(t: &Term) -> Vec<u8> {
    fn part(out: &mut Vec<u8>, s: &str) {
        out.extend_from_slice(&(s.len() as u32).to_be_bytes());
        out.extend_from_slice(s.as_bytes());
    }
    let mut out = Vec::new();
    match t {
        Term::Iri(i) => {
            out.push(0);
            part(&mut out, i);
        }
        Term::Blank(b) => {
            out.push(1);
            part(&mut out, b);
        }
        Term::Literal {
            value,
            datatype,
            language,
        } => {
            out.push(2);
            part(&mut out, value);
            part(&mut out, datatype.as_deref().unwrap_or(""));
            part(&mut out, language.as_deref().unwrap_or(""));
        }
    }
    out
}

fn decode(b: &[u8]) -> Term {
    fn part(b: &[u8], at: &mut usize) -> String {
        let n = u32::from_be_bytes(b[*at..*at + 4].try_into().expect("4 bytes")) as usize;
        let s = String::from_utf8_lossy(&b[*at + 4..*at + 4 + n]).into_owned();
        *at += 4 + n;
        s
    }
    let mut at = 1;
    match b[0] {
        0 => Term::Iri(part(b, &mut at)),
        1 => Term::Blank(part(b, &mut at)),
        _ => {
            let value = part(b, &mut at);
            let d = part(b, &mut at);
            let l = part(b, &mut at);
            Term::Literal {
                value,
                datatype: (!d.is_empty()).then_some(d),
                language: (!l.is_empty()).then_some(l),
            }
        }
    }
}

/// Which sorted order a range comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    Spo,
    Pos,
    Osp,
}

impl Order {
    const ALL: [Order; 3] = [Order::Spo, Order::Pos, Order::Osp];

    /// An `spo` entry `[s, p, o, g]` in this order.
    fn permute(self, [s, p, o, g]: [u32; 4]) -> [u32; 4] {
        match self {
            Order::Spo => [s, p, o, g],
            Order::Pos => [p, o, s, g],
            Order::Osp => [o, s, p, g],
        }
    }
}

/// An entry of `order` as (s, p, o).
pub fn spo(order: Order, e: [u32; 4]) -> [u32; 3] {
    match order {
        Order::Spo => [e[0], e[1], e[2]],
        Order::Pos => [e[2], e[0], e[1]],
        Order::Osp => [e[1], e[2], e[0]],
    }
}

/// Collects every fact of a generation, for a full build's index.
#[derive(Default)]
pub struct Builder {
    terms: HashMap<Vec<u8>, u32>,
    list: Vec<Vec<u8>>,
    entries: Vec<[u32; 4]>,
}

impl Builder {
    fn id(&mut self, e: Vec<u8>) -> u32 {
        if let Some(id) = self.terms.get(&e) {
            return *id;
        }
        let id = self.list.len() as u32;
        self.terms.insert(e.clone(), id);
        self.list.push(e);
        id
    }

    /// One fact of the generation, in `owner`'s graph.
    pub fn add(&mut self, s: &Term, p: &Term, o: &Term, owner: Option<&str>) {
        let g = match owner {
            Some(g) => self.id(encode(&Term::Iri(g.to_string()))),
            None => NO_OWNER,
        };
        let e = [
            self.id(encode(s)),
            self.id(encode(p)),
            self.id(encode(o)),
            g,
        ];
        self.entries.push(e);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Writes the index file at `path`, through a temporary file and a rename.
    pub fn write(self, path: &Path) -> io::Result<()> {
        let Builder {
            list, mut entries, ..
        } = self;
        entries.sort_unstable();
        entries.dedup();
        let dict = Dictionary {
            base: None,
            added: &list,
        };
        write_file(path, &dict, |order| {
            let mut v: Vec<[u32; 4]> = entries.iter().map(|e| order.permute(*e)).collect();
            if order != Order::Spo {
                v.sort_unstable();
            }
            Box::new(v.into_iter())
        })
    }
}

/// Writes the index of a generation made from `base`'s generation by an update: `base`'s entries
/// without those of the owners in `dropped` (`None` for the default graph), and `added`, the
/// current facts of those owners.
pub fn merge(
    base: &QIndex,
    dropped: &BTreeSet<Option<String>>,
    added: impl IntoIterator<Item = (Term, Term, Term, Option<String>)>,
    path: &Path,
) -> io::Result<()> {
    let dropped_ids: std::collections::HashSet<u32> = dropped
        .iter()
        .filter_map(|o| match o {
            Some(o) => base.id(&Term::Iri(o.clone())),
            None => Some(NO_OWNER),
        })
        .collect();
    let mut fresh_ids: HashMap<Vec<u8>, u32> = HashMap::new();
    let mut fresh_terms: Vec<Vec<u8>> = Vec::new();
    let mut id = |e: Vec<u8>| -> u32 {
        if let Some(id) = base.id_encoded(&e) {
            return id;
        }
        if let Some(id) = fresh_ids.get(&e) {
            return *id;
        }
        let id = (base.n + fresh_terms.len()) as u32;
        fresh_ids.insert(e.clone(), id);
        fresh_terms.push(e);
        id
    };
    let mut new: Vec<[u32; 4]> = added
        .into_iter()
        .map(|(s, p, o, g)| {
            let g = match g {
                Some(g) => id(encode(&Term::Iri(g))),
                None => NO_OWNER,
            };
            [id(encode(&s)), id(encode(&p)), id(encode(&o)), g]
        })
        .collect();
    new.sort_unstable();
    new.dedup();
    let dict = Dictionary {
        base: Some(base),
        added: &fresh_terms,
    };
    write_file(path, &dict, |order| {
        let dropped = &dropped_ids;
        let kept = (0..base.m)
            .map(move |i| base.entry(order, i))
            .filter(move |e| !dropped.contains(&e[3]));
        let mut fresh: Vec<[u32; 4]> = new.iter().map(|e| order.permute(*e)).collect();
        fresh.sort_unstable();
        Box::new(MergeSorted::new(kept, fresh.into_iter()))
    })
}

/// Two sorted streams as one, without duplicates.
struct MergeSorted<A: Iterator<Item = [u32; 4]>, B: Iterator<Item = [u32; 4]>> {
    a: std::iter::Peekable<A>,
    b: std::iter::Peekable<B>,
    last: Option<[u32; 4]>,
}

impl<A: Iterator<Item = [u32; 4]>, B: Iterator<Item = [u32; 4]>> MergeSorted<A, B> {
    fn new(a: A, b: B) -> Self {
        MergeSorted {
            a: a.peekable(),
            b: b.peekable(),
            last: None,
        }
    }
}

impl<A: Iterator<Item = [u32; 4]>, B: Iterator<Item = [u32; 4]>> Iterator for MergeSorted<A, B> {
    type Item = [u32; 4];
    fn next(&mut self) -> Option<[u32; 4]> {
        loop {
            let next = match (self.a.peek(), self.b.peek()) {
                (Some(x), Some(y)) => {
                    if x <= y {
                        self.a.next()
                    } else {
                        self.b.next()
                    }
                }
                (Some(_), None) => self.a.next(),
                (None, Some(_)) => self.b.next(),
                (None, None) => return None,
            }?;
            if self.last != Some(next) {
                self.last = Some(next);
                return Some(next);
            }
        }
    }
}

/// The dictionary being written: a base's terms (kept whole, so ids stay), then new ones.
struct Dictionary<'a> {
    base: Option<&'a QIndex>,
    added: &'a [Vec<u8>],
}

impl Dictionary<'_> {
    fn base_len(&self) -> usize {
        self.base.map_or(0, |b| b.n)
    }

    fn len(&self) -> usize {
        self.base_len() + self.added.len()
    }

    fn bytes(&self, id: usize) -> &[u8] {
        match self.base {
            Some(b) if id < b.n => b.term_bytes(id as u32),
            _ => &self.added[id - self.base_len()],
        }
    }

    /// Ids sorted by encoded term: the base's own order merged with the new terms, sorted.
    fn lookup(&self) -> Vec<u32> {
        let base = self.base_len();
        let mut fresh: Vec<u32> = (base as u32..self.len() as u32).collect();
        fresh.sort_unstable_by(|a, b| self.bytes(*a as usize).cmp(self.bytes(*b as usize)));
        let Some(b) = self.base else {
            return fresh;
        };
        let mut out = Vec::with_capacity(self.len());
        let (mut i, mut j) = (0, 0);
        while i < b.n || j < fresh.len() {
            let take_base = j == fresh.len()
                || (i < b.n && b.term_bytes(b.lookup_id(i)) <= self.bytes(fresh[j] as usize));
            if take_base {
                out.push(b.lookup_id(i));
                i += 1;
            } else {
                out.push(fresh[j]);
                j += 1;
            }
        }
        out
    }
}

/// Writes the whole file. `entries(order)` yields that order's entries, sorted.
fn write_file<'d>(
    path: &Path,
    dict: &Dictionary,
    mut entries: impl FnMut(Order) -> Box<dyn Iterator<Item = [u32; 4]> + 'd>,
) -> io::Result<()> {
    let n = dict.len();
    let tmp = path.with_extension("qindex.tmp");
    let mut w = BufWriter::with_capacity(1 << 22, File::create(&tmp)?);
    w.write_all(&[0u8; HEADER])?;
    let mut at = HEADER as u64;
    let mut sections = [0u64; SECTIONS];

    // Terms.
    sections[0] = at;
    let mut off = 0u64;
    for id in 0..n {
        w.write_all(&off.to_le_bytes())?;
        off += dict.bytes(id).len() as u64;
    }
    w.write_all(&off.to_le_bytes())?;
    at += 8 * (n as u64 + 1);
    sections[1] = at;
    for id in 0..n {
        w.write_all(dict.bytes(id))?;
    }
    at += off;
    let pad = at.next_multiple_of(8) - at;
    w.write_all(&vec![0u8; pad as usize])?;
    at += pad;
    sections[2] = at;
    for id in dict.lookup() {
        w.write_all(&id.to_le_bytes())?;
    }
    at += 4 * n as u64;

    // The three orders, with what `starts` and `stats` need, counted on the way.
    let mut m = 0u64;
    let mut starts: Vec<Vec<u32>> = Vec::new();
    // Per predicate: entries, distinct subjects, distinct objects.
    let mut stats: HashMap<u32, [u32; 3]> = HashMap::new();
    for (k, order) in Order::ALL.into_iter().enumerate() {
        sections[3 + k] = at;
        let mut counts = vec![0u32; n + 1];
        let mut count = 0u64;
        let mut last: Option<[u32; 4]> = None;
        for e in entries(order) {
            for x in e {
                w.write_all(&x.to_le_bytes())?;
            }
            counts[e[0] as usize + 1] += 1;
            count += 1;
            let new_pair = last.is_none_or(|l| l[0] != e[0] || l[1] != e[1]);
            match order {
                // (s, p) pairs are runs in `spo`: one more distinct subject of p.
                Order::Spo if new_pair => stats.entry(e[1]).or_default()[1] += 1,
                // (p, o) pairs are runs in `pos`: one more distinct object of p.
                Order::Pos => {
                    let st = stats.entry(e[0]).or_default();
                    st[0] += 1;
                    if new_pair {
                        st[2] += 1;
                    }
                }
                _ => {}
            }
            last = Some(e);
        }
        for i in 1..=n {
            counts[i] += counts[i - 1];
        }
        starts.push(counts);
        at += 16 * count;
        m = count;
    }
    for (k, list) in starts.iter().enumerate() {
        sections[6 + k] = at;
        for x in list {
            w.write_all(&x.to_le_bytes())?;
        }
        at += 4 * list.len() as u64;
    }
    sections[9] = at;
    let mut rows: Vec<(u32, [u32; 3])> = stats.into_iter().collect();
    rows.sort_unstable();
    w.write_all(&(rows.len() as u32).to_le_bytes())?;
    for (p, st) in rows {
        for x in [p, st[0], st[1], st[2]] {
            w.write_all(&x.to_le_bytes())?;
        }
    }
    w.flush()?;
    let mut file = w.into_inner().map_err(|e| e.into_error())?;
    use std::io::Seek;
    file.seek(io::SeekFrom::Start(0))?;
    let mut header = Vec::with_capacity(HEADER);
    header.extend_from_slice(MAGIC);
    header.extend_from_slice(&(n as u32).to_le_bytes());
    header.extend_from_slice(&m.to_le_bytes());
    for s in sections {
        header.extend_from_slice(&s.to_le_bytes());
    }
    file.write_all(&header)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(tmp, path)
}

/// An open index file.
pub struct QIndex {
    map: memmap2::Mmap,
    n: usize,
    m: usize,
    sections: [usize; SECTIONS],
    predicates: usize,
}

impl QIndex {
    pub fn open(path: &Path) -> io::Result<QIndex> {
        let file = File::open(path)?;
        // SAFETY: an index file is written once, through a temporary file and a rename, and
        // never changed afterwards (spec 006 G4), so the mapped bytes do not change under it.
        let map = unsafe { memmap2::Mmap::map(&file)? };
        let bad = || io::Error::new(io::ErrorKind::InvalidData, "not a query index");
        if map.len() < HEADER || &map[..8] != MAGIC {
            return Err(bad());
        }
        let u64_at = |i: usize| u64::from_le_bytes(map[i..i + 8].try_into().expect("8")) as usize;
        let n = u32::from_le_bytes(map[8..12].try_into().expect("4")) as usize;
        let m = u64_at(12);
        let mut sections = [0usize; SECTIONS];
        for (k, s) in sections.iter_mut().enumerate() {
            *s = u64_at(20 + 8 * k);
        }
        let stats = sections[9];
        if stats + 4 > map.len() {
            return Err(bad());
        }
        let predicates = u32::from_le_bytes(map[stats..stats + 4].try_into().expect("4")) as usize;
        let sizes_agree = stats + 4 + 16 * predicates == map.len()
            && sections[6] == sections[5] + 16 * m
            && sections[9] == sections[8] + 4 * (n + 1);
        if !sizes_agree {
            return Err(bad());
        }
        Ok(QIndex {
            map,
            n,
            m,
            sections,
            predicates,
        })
    }

    pub fn terms(&self) -> usize {
        self.n
    }

    pub fn facts(&self) -> usize {
        self.m
    }

    fn u32_at(&self, at: usize) -> u32 {
        u32::from_le_bytes(self.map[at..at + 4].try_into().expect("4"))
    }

    fn u64_at(&self, at: usize) -> usize {
        u64::from_le_bytes(self.map[at..at + 8].try_into().expect("8")) as usize
    }

    fn term_bytes(&self, id: u32) -> &[u8] {
        let i = id as usize;
        let a = self.u64_at(self.sections[0] + 8 * i);
        let b = self.u64_at(self.sections[0] + 8 * (i + 1));
        &self.map[self.sections[1] + a..self.sections[1] + b]
    }

    fn lookup_id(&self, i: usize) -> u32 {
        self.u32_at(self.sections[2] + 4 * i)
    }

    /// Whether term `id` is a literal, from its kind byte alone.
    pub fn is_literal(&self, id: u32) -> bool {
        self.term_bytes(id).first() == Some(&2)
    }

    pub fn term(&self, id: u32) -> Term {
        decode(self.term_bytes(id))
    }

    fn id_encoded(&self, want: &[u8]) -> Option<u32> {
        let (mut lo, mut hi) = (0usize, self.n);
        while lo < hi {
            let mid = (lo + hi) / 2;
            let id = self.lookup_id(mid);
            match self.term_bytes(id).cmp(want) {
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
                std::cmp::Ordering::Equal => return Some(id),
            }
        }
        None
    }

    /// The id of `t`, when the generation holds it.
    pub fn id(&self, t: &Term) -> Option<u32> {
        self.id_encoded(&encode(t))
    }

    /// Entry `i` of an order, as stored (for `Pos`: p, o, s, owner).
    #[inline]
    pub fn entry(&self, order: Order, i: usize) -> [u32; 4] {
        let base = self.sections[3 + order as usize] + 16 * i;
        let b = &self.map[base..base + 16];
        [0, 4, 8, 12].map(|k| u32::from_le_bytes(b[k..k + 4].try_into().expect("4")))
    }

    /// A predicate's entries, distinct subjects and distinct objects.
    pub fn stats(&self, p: u32) -> Option<[u32; 3]> {
        let row = |i: usize| -> [u32; 4] {
            let at = self.sections[9] + 4 + 16 * i;
            let b = &self.map[at..at + 16];
            [0, 4, 8, 12].map(|k| u32::from_le_bytes(b[k..k + 4].try_into().expect("4")))
        };
        let (mut lo, mut hi) = (0, self.predicates);
        while lo < hi {
            let mid = (lo + hi) / 2;
            let r = row(mid);
            match r[0].cmp(&p) {
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
                std::cmp::Ordering::Equal => return Some([r[1], r[2], r[3]]),
            }
        }
        None
    }

    fn start(&self, order: Order, id: usize) -> usize {
        self.u32_at(self.sections[6 + order as usize] + 4 * id) as usize
    }

    /// The size of the block of entries of `order` whose first component is `id`.
    pub fn block(&self, order: Order, id: u32) -> usize {
        let id = id as usize;
        if id >= self.n {
            return 0;
        }
        self.start(order, id + 1) - self.start(order, id)
    }

    /// The index range of `order` whose first `prefix.len()` components equal `prefix`.
    pub fn range(&self, order: Order, prefix: &[u32]) -> std::ops::Range<usize> {
        let Some(first) = prefix.first() else {
            return 0..self.m;
        };
        let first = *first as usize;
        if first >= self.n {
            return 0..0;
        }
        let (mut lo, end) = (self.start(order, first), self.start(order, first + 1));
        if prefix.len() == 1 || lo == end {
            return lo..end;
        }
        // Within the block, every entry has the same first component.
        let k = prefix.len();
        let key = |i: usize| self.entry(order, i)[1..k].cmp(&prefix[1..]);
        let mut hi = end;
        while lo < hi {
            let mid = (lo + hi) / 2;
            if key(mid) == std::cmp::Ordering::Less {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        let mut hi = end;
        while lo < hi {
            let mid = (lo + hi) / 2;
            if key(mid) == std::cmp::Ordering::Greater {
                hi = mid;
            } else {
                lo = mid + 1;
            }
        }
        start..lo
    }
}

/// Builds the index of the store's current generation from its facts, read through SPARQL. For
/// tests and small stores; builds and updates write it as they publish.
pub fn build_from_reader(reader: &crate::graph::Reader, dir: &Path) -> io::Result<()> {
    let rows = reader
        .select("SELECT ?s ?p ?o ?g WHERE { GRAPH ?g { ?s ?p ?o } }")
        .map_err(|e| io::Error::other(e.to_string()))?
        .rows;
    let mut b = Builder::default();
    for row in rows {
        if let [Some(s), Some(p), Some(o), g] = &row[..] {
            let owner = match g {
                Some(Term::Iri(g)) => Some(g.as_str()),
                _ => None,
            };
            b.add(s, p, o, owner);
        }
    }
    b.write(&path(dir, reader.generation().number))
}
