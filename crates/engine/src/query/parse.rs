//! The parser for query language level 1 (spec 009 contracts/query-language.md; research R1).
//! It accepts one `SELECT` over one group of triple patterns and comparisons, and refuses every
//! other SPARQL construct by name, at its position.

use super::error::{Pos, QueryError};
use super::lex::{tokens, Tok, Token};
use super::names::BUILT_IN_PREFIXES;

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const XSD: &str = "http://www.w3.org/2001/XMLSchema#";

/// An IRI as written: in full, or as a prefixed name (local part still percent-encoded).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IriRef {
    Full(String),
    Prefixed { prefix: String, local: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    Var(String),
    Iri(IriRef),
    Literal {
        lexical: String,
        datatype: Option<IriRef>,
        language: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub kind: NodeKind,
    pub pos: Pos,
}

impl Node {
    pub fn var(&self) -> Option<&str> {
        match &self.kind {
            NodeKind::Var(v) => Some(v),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriplePattern {
    pub s: Node,
    pub p: Node,
    pub o: Node,
    pub pos: Pos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl CompareOp {
    pub fn as_str(self) -> &'static str {
        match self {
            CompareOp::Eq => "=",
            CompareOp::Ne => "!=",
            CompareOp::Lt => "<",
            CompareOp::Le => "<=",
            CompareOp::Gt => ">",
            CompareOp::Ge => ">=",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    pub left: Node,
    pub op: CompareOp,
    pub right: Node,
    pub pos: Pos,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Projection {
    All,
    Vars(Vec<(String, Pos)>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ast {
    /// User `PREFIX` lines: name, IRI, position.
    pub prefixes: Vec<(String, String, Pos)>,
    pub distinct: bool,
    pub projection: Projection,
    pub patterns: Vec<TriplePattern>,
    pub filters: Vec<Filter>,
}

impl Ast {
    /// The projected variables: as listed, or for `*` every variable in order of first
    /// appearance.
    pub fn variables(&self) -> Vec<String> {
        match &self.projection {
            Projection::Vars(vs) => vs.iter().map(|(v, _)| v.clone()).collect(),
            Projection::All => {
                let mut out: Vec<String> = Vec::new();
                for t in &self.patterns {
                    for n in [&t.s, &t.p, &t.o] {
                        if let Some(v) = n.var() {
                            if !out.iter().any(|x| x == v) {
                                out.push(v.to_string());
                            }
                        }
                    }
                }
                out
            }
        }
    }
}

/// Keywords that start a refused construct, with the name the error gives.
fn refused_keyword(word: &str) -> Option<&'static str> {
    Some(match word {
        "ASK" => "ASK",
        "CONSTRUCT" => "CONSTRUCT",
        "DESCRIBE" => "DESCRIBE",
        "INSERT" | "DELETE" | "LOAD" | "CLEAR" | "CREATE" | "DROP" | "COPY" | "MOVE" | "ADD"
        | "WITH" => "an update",
        "BASE" => "BASE",
        "OPTIONAL" => "OPTIONAL",
        "UNION" => "UNION",
        "MINUS" => "MINUS",
        "GRAPH" => "GRAPH",
        "SERVICE" => "SERVICE",
        "BIND" => "BIND",
        "VALUES" => "VALUES",
        "GROUP" => "GROUP BY",
        "HAVING" => "HAVING",
        "ORDER" => "ORDER BY",
        "LIMIT" => "LIMIT",
        "OFFSET" => "OFFSET",
        "FROM" => "FROM",
        "REDUCED" => "REDUCED",
        "COUNT" | "SUM" | "MIN" | "MAX" | "AVG" | "SAMPLE" | "GROUP_CONCAT" => "aggregates",
        "NOT" | "EXISTS" => "EXISTS",
        _ => return None,
    })
}

struct Parser {
    toks: Vec<Token>,
    i: usize,
    end: Pos,
}

fn describe(t: &Tok) -> String {
    match t {
        Tok::Iri(i) => format!("<{i}>"),
        Tok::PName { prefix, local } => format!("{prefix}:{local}"),
        Tok::Var(v) => format!("?{v}"),
        Tok::Str(_) => "a string".into(),
        Tok::Lang(l) => format!("@{l}"),
        Tok::Integer(n) | Tok::Decimal(n) | Tok::Double(n) => n.clone(),
        Tok::BNode(b) => format!("_:{b}"),
        Tok::Word(w) => w.clone(),
        Tok::Punct(p) => format!("'{p}'"),
    }
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.toks.get(self.i)
    }

    fn peek_tok(&self) -> Option<&Tok> {
        self.peek().map(|t| &t.tok)
    }

    fn here(&self) -> Pos {
        self.peek().map(|t| t.pos).unwrap_or(self.end)
    }

    fn next(&mut self) -> Option<Token> {
        let t = self.toks.get(self.i).cloned();
        if t.is_some() {
            self.i += 1;
        }
        t
    }

    fn is_punct(&self, p: &str) -> bool {
        matches!(self.peek_tok(), Some(Tok::Punct(q)) if *q == p)
    }

    fn is_word(&self, w: &str) -> bool {
        matches!(self.peek_tok(), Some(Tok::Word(x)) if x == w)
    }

    fn unexpected(&self, wanted: &str) -> QueryError {
        match self.peek() {
            Some(t) => QueryError::syntax(
                format!("expected {wanted}, found {}", describe(&t.tok)),
                t.pos,
            ),
            None => QueryError::syntax(format!("expected {wanted}, but the query ends"), self.end),
        }
    }

    fn expect_punct(&mut self, p: &str) -> Result<Pos, QueryError> {
        if self.is_punct(p) {
            Ok(self.next().unwrap().pos)
        } else {
            Err(self.unexpected(&format!("'{p}'")))
        }
    }

    /// Refuses the current token when it starts a refused construct.
    fn refuse_here(&self) -> Result<(), QueryError> {
        let Some(t) = self.peek() else { return Ok(()) };
        match &t.tok {
            Tok::Word(w) => {
                if let Some(c) = refused_keyword(w) {
                    return Err(QueryError::unsupported(c, t.pos));
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn query(&mut self) -> Result<Ast, QueryError> {
        let mut prefixes = Vec::new();
        loop {
            if self.is_word("PREFIX") {
                let at = self.next().unwrap().pos;
                let name = match self.next() {
                    Some(Token {
                        tok: Tok::PName { prefix, local },
                        ..
                    }) if local.is_empty() => prefix,
                    _ => return Err(QueryError::syntax("PREFIX needs a name ending in ':'", at)),
                };
                if BUILT_IN_PREFIXES.contains(&name.as_str()) {
                    return Err(QueryError::syntax(
                        format!("the prefix {name}: is built in and cannot be redefined"),
                        at,
                    ));
                }
                let iri = match self.next() {
                    Some(Token {
                        tok: Tok::Iri(i), ..
                    }) => i,
                    _ => return Err(QueryError::syntax("PREFIX needs an <IRI>", at)),
                };
                prefixes.push((name, iri, at));
            } else {
                self.refuse_here()?;
                break;
            }
        }
        if !self.is_word("SELECT") {
            return Err(self.unexpected("SELECT"));
        }
        self.next();
        let mut distinct = false;
        if self.is_word("DISTINCT") {
            self.next();
            distinct = true;
        }
        self.refuse_here()?;
        let projection = if self.is_punct("*") {
            self.next();
            Projection::All
        } else {
            let mut vars = Vec::new();
            loop {
                match self.peek().cloned() {
                    Some(Token {
                        tok: Tok::Var(v),
                        pos,
                    }) => {
                        self.next();
                        vars.push((v, pos));
                    }
                    Some(Token {
                        tok: Tok::Punct("("),
                        pos,
                    }) => return Err(QueryError::unsupported("expressions in SELECT", pos)),
                    _ => break,
                }
            }
            if vars.is_empty() {
                return Err(self.unexpected("a variable or '*' after SELECT"));
            }
            Projection::Vars(vars)
        };
        self.refuse_here()?;
        if self.is_word("WHERE") {
            self.next();
        }
        let (patterns, filters) = self.group()?;
        if self.peek().is_some() {
            self.refuse_here()?;
            return Err(self.unexpected("the end of the query"));
        }
        let ast = Ast {
            prefixes,
            distinct,
            projection,
            patterns,
            filters,
        };
        check(&ast)?;
        Ok(ast)
    }

    fn group(&mut self) -> Result<(Vec<TriplePattern>, Vec<Filter>), QueryError> {
        self.expect_punct("{")?;
        let mut patterns = Vec::new();
        let mut filters = Vec::new();
        loop {
            if self.is_punct("}") {
                self.next();
                return Ok((patterns, filters));
            }
            if self.peek().is_none() {
                return Err(self.unexpected("'}'"));
            }
            if self.is_punct("{") {
                return Err(QueryError::unsupported("a nested group", self.here()));
            }
            if self.is_word("FILTER") {
                filters.push(self.filter()?);
                if self.is_punct(".") {
                    self.next();
                }
                continue;
            }
            self.refuse_here()?;
            if self.is_word("SELECT") {
                return Err(QueryError::unsupported("a sub-query", self.here()));
            }
            self.triples(&mut patterns)?;
            if self.is_punct(".") {
                self.next();
            } else if self.peek().is_none() {
                return Err(self.unexpected("'}'"));
            } else if !(self.is_punct("}") || self.is_word("FILTER")) {
                self.refuse_here()?;
                return Err(self.unexpected("'.' between triple patterns"));
            }
        }
    }

    fn triples(&mut self, out: &mut Vec<TriplePattern>) -> Result<(), QueryError> {
        let s = self.term("a subject")?;
        if matches!(s.kind, NodeKind::Literal { .. }) {
            return Err(QueryError::syntax("a literal cannot be a subject", s.pos));
        }
        loop {
            let p = self.verb()?;
            loop {
                let o = self.term("an object")?;
                out.push(TriplePattern {
                    s: s.clone(),
                    p: p.clone(),
                    pos: s.pos,
                    o,
                });
                if self.is_punct(",") {
                    self.next();
                } else {
                    break;
                }
            }
            if self.is_punct(";") {
                while self.is_punct(";") {
                    self.next();
                }
                if self.is_punct(".") || self.is_punct("}") || self.is_word("FILTER") {
                    return Ok(());
                }
            } else {
                return Ok(());
            }
        }
    }

    fn verb(&mut self) -> Result<Node, QueryError> {
        let at = self.here();
        match self.peek_tok() {
            Some(Tok::Punct("^" | "!" | "(")) => {
                return Err(QueryError::unsupported("property paths", at))
            }
            Some(Tok::Word(w)) if w == "a" => {
                self.next();
                let n = Node {
                    kind: NodeKind::Iri(IriRef::Full(RDF_TYPE.into())),
                    pos: at,
                };
                self.no_path()?;
                return Ok(n);
            }
            _ => {}
        }
        let n = self.term("a predicate")?;
        if matches!(n.kind, NodeKind::Literal { .. }) {
            return Err(QueryError::syntax("a literal cannot be a predicate", n.pos));
        }
        self.no_path()?;
        Ok(n)
    }

    /// A path operator right after a verb.
    fn no_path(&self) -> Result<(), QueryError> {
        match self.peek() {
            Some(Token {
                tok: Tok::Punct("/" | "|" | "*" | "+" | "?" | "^"),
                pos,
            }) => Err(QueryError::unsupported("property paths", *pos)),
            _ => Ok(()),
        }
    }

    fn iri_ref(&mut self, wanted: &str) -> Result<IriRef, QueryError> {
        match self.next() {
            Some(Token {
                tok: Tok::Iri(i), ..
            }) => Ok(IriRef::Full(i)),
            Some(Token {
                tok: Tok::PName { prefix, local },
                ..
            }) => Ok(IriRef::Prefixed { prefix, local }),
            _ => {
                self.i -= 1;
                Err(self.unexpected(wanted))
            }
        }
    }

    fn term(&mut self, wanted: &str) -> Result<Node, QueryError> {
        let Some(t) = self.peek().cloned() else {
            return Err(self.unexpected(wanted));
        };
        let pos = t.pos;
        let typed = |lexical: String, dt: &str| NodeKind::Literal {
            lexical,
            datatype: Some(IriRef::Full(format!("{XSD}{dt}"))),
            language: None,
        };
        let kind = match t.tok {
            Tok::Var(v) => {
                self.next();
                NodeKind::Var(v)
            }
            Tok::Iri(_) | Tok::PName { .. } => NodeKind::Iri(self.iri_ref(wanted)?),
            Tok::Str(s) => {
                self.next();
                let mut language = None;
                let mut datatype = None;
                match self.peek_tok() {
                    Some(Tok::Lang(l)) => {
                        language = Some(l.clone());
                        self.next();
                    }
                    Some(Tok::Punct("^^")) => {
                        self.next();
                        datatype = Some(self.iri_ref("a datatype after ^^")?);
                    }
                    _ => {}
                }
                NodeKind::Literal {
                    lexical: s,
                    datatype,
                    language,
                }
            }
            Tok::Integer(n) => {
                self.next();
                typed(n, "integer")
            }
            Tok::Decimal(n) => {
                self.next();
                typed(n, "decimal")
            }
            Tok::Double(n) => {
                self.next();
                typed(n, "double")
            }
            Tok::Word(w) if w == "TRUE" || w == "FALSE" => {
                self.next();
                typed(w.to_ascii_lowercase(), "boolean")
            }
            Tok::BNode(_) | Tok::Punct("[") => {
                return Err(QueryError::unsupported("blank nodes", pos))
            }
            Tok::Punct("(") => return Err(QueryError::unsupported("collections", pos)),
            Tok::Punct("{") => return Err(QueryError::unsupported("a nested group", pos)),
            Tok::Word(w) if refused_keyword(&w).is_some() => {
                self.refuse_here()?;
                unreachable!()
            }
            _ => return Err(self.unexpected(wanted)),
        };
        Ok(Node { kind, pos })
    }

    fn filter(&mut self) -> Result<Filter, QueryError> {
        let at = self.next().unwrap().pos;
        match self.peek() {
            Some(Token {
                tok: Tok::Punct("("),
                ..
            }) => {
                self.next();
            }
            Some(Token {
                tok: Tok::Word(w),
                pos,
            }) => {
                let c = refused_keyword(w)
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("the function {w}"));
                return Err(QueryError::unsupported(c, *pos));
            }
            _ => return Err(self.unexpected("'(' after FILTER")),
        }
        let left = self.operand()?;
        let op_at = self.here();
        let op = match self.next().map(|t| t.tok) {
            Some(Tok::Punct("=")) => CompareOp::Eq,
            Some(Tok::Punct("!=")) => CompareOp::Ne,
            Some(Tok::Punct("<")) => CompareOp::Lt,
            Some(Tok::Punct("<=")) => CompareOp::Le,
            Some(Tok::Punct(">")) => CompareOp::Gt,
            Some(Tok::Punct(">=")) => CompareOp::Ge,
            Some(Tok::Punct(p @ ("&&" | "||"))) => return Err(QueryError::unsupported(p, op_at)),
            Some(Tok::Punct("+" | "-" | "*" | "/")) => {
                return Err(QueryError::unsupported("arithmetic", op_at))
            }
            Some(Tok::Word(w)) if w == "IN" || w == "NOT" => {
                return Err(QueryError::unsupported("IN", op_at))
            }
            _ => {
                self.i -= 1;
                return Err(self.unexpected("a comparison (= != < <= > >=)"));
            }
        };
        let right = self.operand()?;
        match self.peek() {
            Some(Token {
                tok: Tok::Punct(")"),
                ..
            }) => {
                self.next();
            }
            Some(Token {
                tok: Tok::Punct(p @ ("&&" | "||")),
                pos,
            }) => return Err(QueryError::unsupported(*p, *pos)),
            Some(Token {
                tok: Tok::Punct("+" | "-" | "*" | "/" | "=" | "!=" | "<" | "<=" | ">" | ">="),
                pos,
            }) => return Err(QueryError::unsupported("arithmetic", *pos)),
            _ => return Err(self.unexpected("')'")),
        }
        Ok(Filter {
            left,
            op,
            right,
            pos: at,
        })
    }

    fn operand(&mut self) -> Result<Node, QueryError> {
        match self.peek() {
            Some(Token {
                tok: Tok::Punct("!"),
                pos,
            }) => Err(QueryError::unsupported("!", *pos)),
            Some(Token {
                tok: Tok::Punct("("),
                pos,
            }) => Err(QueryError::unsupported("nested expressions", *pos)),
            Some(Token {
                tok: Tok::Punct(p @ ("+" | "-" | "*" | "/")),
                pos,
            }) => {
                let _ = p;
                Err(QueryError::unsupported("arithmetic", *pos))
            }
            Some(Token {
                tok: Tok::Word(w),
                pos,
            }) if w != "TRUE" && w != "FALSE" => {
                let c = refused_keyword(w)
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("the function {w}"));
                Err(QueryError::unsupported(c, *pos))
            }
            Some(Token {
                tok: Tok::PName { .. } | Tok::Iri(_),
                ..
            }) if matches!(
                self.toks.get(self.i + 1).map(|t| &t.tok),
                Some(Tok::Punct("("))
            ) =>
            {
                let pos = self.here();
                Err(QueryError::unsupported("function calls", pos))
            }
            _ => self.term("a value to compare"),
        }
    }
}

/// The checks after parsing: projected variables appear in a pattern, and no user variable
/// starts with the engine's reserved `__se`.
fn check(ast: &Ast) -> Result<(), QueryError> {
    let used = |v: &str| {
        ast.patterns
            .iter()
            .any(|t| [&t.s, &t.p, &t.o].iter().any(|n| n.var() == Some(v)))
    };
    if let Projection::Vars(vs) = &ast.projection {
        if let Some((_, pos)) = vs.iter().find(|(v, _)| v.starts_with(super::HIDDEN)) {
            return Err(QueryError::syntax(
                format!("variables starting {} are reserved", super::HIDDEN),
                *pos,
            ));
        }
    }
    let mut nodes: Vec<&Node> = Vec::new();
    for t in &ast.patterns {
        nodes.extend([&t.s, &t.p, &t.o]);
    }
    for f in &ast.filters {
        nodes.extend([&f.left, &f.right]);
    }
    for n in nodes {
        if let Some(v) = n.var() {
            if v.starts_with(super::HIDDEN) {
                return Err(QueryError::syntax(
                    format!("variables starting {} are reserved", super::HIDDEN),
                    n.pos,
                ));
            }
        }
    }
    if let Projection::Vars(vs) = &ast.projection {
        for (v, pos) in vs {
            if !used(v) {
                return Err(QueryError::syntax(
                    format!("?{v} is selected but not used in any triple pattern"),
                    *pos,
                ));
            }
        }
    }
    for f in &ast.filters {
        for n in [&f.left, &f.right] {
            if let Some(v) = n.var() {
                if !used(v) {
                    return Err(QueryError::syntax(
                        format!("?{v} is compared but not used in any triple pattern"),
                        n.pos,
                    ));
                }
            }
        }
    }
    if ast.patterns.is_empty() {
        return Err(QueryError::syntax(
            "the query needs at least one triple pattern",
            Pos { line: 1, column: 1 },
        ));
    }
    Ok(())
}

/// Parses one query of language level 1.
pub fn parse(text: &str) -> Result<Ast, QueryError> {
    let toks = tokens(text)?;
    let (line, column) = text.lines().enumerate().last().map_or((1, 1), |(i, l)| {
        (i as u32 + 1, l.chars().count() as u32 + 1)
    });
    Parser {
        toks,
        i: 0,
        end: Pos { line, column },
    }
    .query()
}
