//! The tokenizer (spec 009 research R1). It reads SPARQL 1.1's terminals, with a position on
//! every token, and knows every SPARQL keyword, so the parser can name a refused construct
//! instead of reporting a generic parse error.

use super::error::{Pos, QueryError};

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    /// `<…>`, without the brackets.
    Iri(String),
    /// `prefix:local`, the local part still percent-encoded; `prefix` may be empty.
    PName {
        prefix: String,
        local: String,
    },
    /// `?name` or `$name`, without the sigil.
    Var(String),
    /// A string literal, unescaped.
    Str(String),
    /// `@en`, without the `@`.
    Lang(String),
    Integer(String),
    Decimal(String),
    Double(String),
    /// `_:label`.
    BNode(String),
    /// A bare word: a keyword in upper case, `a` as written, or any other name.
    Word(String),
    /// Punctuation and operators: `{ } ( ) [ ] . ; , ^^ = != < <= > >= && || ! + - * / | ^ ?`.
    Punct(&'static str),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub tok: Tok,
    pub pos: Pos,
}

/// SPARQL 1.1's keywords and built-in function names, in upper case.
pub const KEYWORDS: &[&str] = &[
    "SELECT",
    "DISTINCT",
    "REDUCED",
    "WHERE",
    "PREFIX",
    "BASE",
    "ASK",
    "CONSTRUCT",
    "DESCRIBE",
    "INSERT",
    "DELETE",
    "LOAD",
    "CLEAR",
    "CREATE",
    "DROP",
    "COPY",
    "MOVE",
    "ADD",
    "WITH",
    "DATA",
    "USING",
    "DEFAULT",
    "ALL",
    "SILENT",
    "TO",
    "INTO",
    "OPTIONAL",
    "UNION",
    "MINUS",
    "GRAPH",
    "SERVICE",
    "BIND",
    "VALUES",
    "FILTER",
    "GROUP",
    "BY",
    "HAVING",
    "ORDER",
    "ASC",
    "DESC",
    "LIMIT",
    "OFFSET",
    "FROM",
    "NAMED",
    "AS",
    "UNDEF",
    "NOT",
    "EXISTS",
    "IN",
    "TRUE",
    "FALSE",
    "COUNT",
    "SUM",
    "MIN",
    "MAX",
    "AVG",
    "SAMPLE",
    "GROUP_CONCAT",
    "SEPARATOR",
];

struct Lexer {
    chars: Vec<char>,
    i: usize,
    line: u32,
    column: u32,
}

fn is_pn_chars_base(c: char) -> bool {
    c.is_alphabetic()
}

fn is_pn_chars(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-' || c == '\u{B7}'
}

fn is_iri_char(c: char) -> bool {
    !(c <= ' ' || matches!(c, '<' | '>' | '"' | '{' | '}' | '|' | '^' | '`' | '\\'))
}

impl Lexer {
    fn peek(&self, k: usize) -> Option<char> {
        self.chars.get(self.i + k).copied()
    }

    fn pos(&self) -> Pos {
        Pos {
            line: self.line,
            column: self.column,
        }
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek(0)?;
        self.i += 1;
        if c == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(c)
    }

    fn skip_space(&mut self) {
        while let Some(c) = self.peek(0) {
            if c.is_whitespace() {
                self.bump();
            } else if c == '#' {
                while let Some(c) = self.peek(0) {
                    if c == '\n' {
                        break;
                    }
                    self.bump();
                }
            } else {
                break;
            }
        }
    }

    /// The length of an `IRIREF` starting at the current `<`, or `None` when it is not one.
    fn iri_len(&self) -> Option<usize> {
        let mut k = 1;
        loop {
            match self.peek(k)? {
                '>' => return Some(k + 1),
                c if is_iri_char(c) => k += 1,
                _ => return None,
            }
        }
    }

    fn name_while(&mut self, f: impl Fn(char) -> bool) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek(0) {
            if f(c) {
                s.push(c);
                self.bump();
            } else {
                break;
            }
        }
        s
    }

    /// A prefix name: `PN_CHARS_BASE ((PN_CHARS | '.')* PN_CHARS)?`, or nothing.
    fn prefix(&mut self) -> String {
        let mut s = String::new();
        if !self.peek(0).is_some_and(is_pn_chars_base) {
            return s;
        }
        while let Some(c) = self.peek(0) {
            if is_pn_chars(c) || (c == '.' && self.peek(1).is_some_and(is_pn_chars)) {
                s.push(c);
                self.bump();
            } else {
                break;
            }
        }
        s
    }

    /// A local name after `prefix:`, kept with its `%XX` escapes; `\x` escapes are unescaped.
    fn local(&mut self) -> Result<String, QueryError> {
        let mut s = String::new();
        while let Some(c) = self.peek(0) {
            if is_pn_chars(c) || c == ':' {
                s.push(c);
                self.bump();
            } else if c == '%' {
                let ok = self.peek(1).is_some_and(|h| h.is_ascii_hexdigit())
                    && self.peek(2).is_some_and(|h| h.is_ascii_hexdigit());
                if !ok {
                    return Err(QueryError::syntax(
                        "a % in a name must be followed by two hex digits",
                        self.pos(),
                    ));
                }
                for _ in 0..3 {
                    s.push(self.bump().unwrap());
                }
            } else if c == '\\' {
                match self.peek(1) {
                    Some(e) if "_~.-!$&'()*+,;=/?#@%".contains(e) => {
                        self.bump();
                        self.bump();
                        if e == '%' {
                            // Keep an escaped percent apart from a percent escape.
                            s.push_str("%25");
                        } else {
                            s.push(e);
                        }
                    }
                    _ => {
                        return Err(QueryError::syntax(
                            "a \\ in a name must escape punctuation",
                            self.pos(),
                        ))
                    }
                }
            } else if c == '.'
                && self
                    .peek(1)
                    .is_some_and(|n| is_pn_chars(n) || n == ':' || n == '%')
            {
                s.push(c);
                self.bump();
            } else {
                break;
            }
        }
        Ok(s)
    }

    fn string(&mut self, quote: char, start: Pos) -> Result<String, QueryError> {
        let long = self.peek(1) == Some(quote) && self.peek(2) == Some(quote);
        let open = if long { 3 } else { 1 };
        for _ in 0..open {
            self.bump();
        }
        let mut s = String::new();
        loop {
            let Some(c) = self.peek(0) else {
                return Err(QueryError::syntax("this string is never closed", start));
            };
            if c == quote {
                if !long {
                    self.bump();
                    return Ok(s);
                }
                if self.peek(1) == Some(quote) && self.peek(2) == Some(quote) {
                    for _ in 0..3 {
                        self.bump();
                    }
                    return Ok(s);
                }
            }
            if !long && (c == '\n' || c == '\r') {
                return Err(QueryError::syntax("this string is never closed", start));
            }
            if c == '\\' {
                let at = self.pos();
                self.bump();
                let e = self.bump();
                match e {
                    Some('t') => s.push('\t'),
                    Some('n') => s.push('\n'),
                    Some('r') => s.push('\r'),
                    Some('b') => s.push('\u{8}'),
                    Some('f') => s.push('\u{C}'),
                    Some('"') => s.push('"'),
                    Some('\'') => s.push('\''),
                    Some('\\') => s.push('\\'),
                    Some(u @ ('u' | 'U')) => {
                        let n = if u == 'u' { 4 } else { 8 };
                        let mut hex = String::new();
                        for _ in 0..n {
                            match self.bump() {
                                Some(h) if h.is_ascii_hexdigit() => hex.push(h),
                                _ => return Err(QueryError::syntax("a bad \\u escape", at)),
                            }
                        }
                        match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                            Some(ch) => s.push(ch),
                            None => return Err(QueryError::syntax("a bad \\u escape", at)),
                        }
                    }
                    _ => return Err(QueryError::syntax("an unknown escape in a string", at)),
                }
                continue;
            }
            s.push(c);
            self.bump();
        }
    }

    fn number(&mut self) -> Tok {
        let mut s = String::new();
        if matches!(self.peek(0), Some('+' | '-')) {
            s.push(self.bump().unwrap());
        }
        s.push_str(&self.name_while(|c| c.is_ascii_digit()));
        let mut kind = 0; // 0 integer, 1 decimal, 2 double
        if self.peek(0) == Some('.') && self.peek(1).is_some_and(|c| c.is_ascii_digit()) {
            s.push(self.bump().unwrap());
            s.push_str(&self.name_while(|c| c.is_ascii_digit()));
            kind = 1;
        }
        if matches!(self.peek(0), Some('e' | 'E')) {
            let sign = matches!(self.peek(1), Some('+' | '-'));
            let digit_at = if sign { 2 } else { 1 };
            if self.peek(digit_at).is_some_and(|c| c.is_ascii_digit()) {
                for _ in 0..digit_at {
                    s.push(self.bump().unwrap());
                }
                s.push_str(&self.name_while(|c| c.is_ascii_digit()));
                kind = 2;
            }
        }
        match kind {
            0 => Tok::Integer(s),
            1 => Tok::Decimal(s),
            _ => Tok::Double(s),
        }
    }

    fn next(&mut self) -> Result<Option<Token>, QueryError> {
        self.skip_space();
        let pos = self.pos();
        let Some(c) = self.peek(0) else {
            return Ok(None);
        };
        let tok = match c {
            '<' => {
                if let Some(n) = self.iri_len() {
                    let mut s = String::new();
                    self.bump();
                    for _ in 1..n - 1 {
                        s.push(self.bump().unwrap());
                    }
                    self.bump();
                    Tok::Iri(s)
                } else if self.peek(1) == Some('=') {
                    self.bump();
                    self.bump();
                    Tok::Punct("<=")
                } else {
                    self.bump();
                    Tok::Punct("<")
                }
            }
            '?' | '$' => {
                if self
                    .peek(1)
                    .is_some_and(|c| c.is_alphanumeric() || c == '_')
                {
                    self.bump();
                    Tok::Var(self.name_while(|c| c.is_alphanumeric() || c == '_' || c == '\u{B7}'))
                } else {
                    self.bump();
                    Tok::Punct("?")
                }
            }
            '"' | '\'' => Tok::Str(self.string(c, pos)?),
            '@' => {
                self.bump();
                let tag = self.name_while(|c| c.is_ascii_alphanumeric() || c == '-');
                if tag.is_empty() {
                    return Err(QueryError::syntax("a language tag is empty", pos));
                }
                Tok::Lang(tag)
            }
            '_' if self.peek(1) == Some(':') => {
                self.bump();
                self.bump();
                Tok::BNode(self.name_while(is_pn_chars))
            }
            '0'..='9' => self.number(),
            '+' | '-' if self.peek(1).is_some_and(|c| c.is_ascii_digit()) => self.number(),
            '.' if self.peek(1).is_some_and(|c| c.is_ascii_digit()) => self.number(),
            ':' => {
                self.bump();
                Tok::PName {
                    prefix: String::new(),
                    local: self.local()?,
                }
            }
            c if is_pn_chars_base(c) => {
                let word = self.prefix();
                if self.peek(0) == Some(':') {
                    self.bump();
                    Tok::PName {
                        prefix: word,
                        local: self.local()?,
                    }
                } else {
                    // A keyword, `a`, or a function name.
                    let upper = word.to_ascii_uppercase();
                    if KEYWORDS.contains(&upper.as_str()) {
                        Tok::Word(upper)
                    } else {
                        Tok::Word(word)
                    }
                }
            }
            _ => {
                let two: String = [Some(c), self.peek(1)].iter().flatten().collect();
                let p: &'static str = match two.as_str() {
                    "^^" => "^^",
                    "!=" => "!=",
                    ">=" => ">=",
                    "&&" => "&&",
                    "||" => "||",
                    _ => match c {
                        '{' => "{",
                        '}' => "}",
                        '(' => "(",
                        ')' => ")",
                        '[' => "[",
                        ']' => "]",
                        '.' => ".",
                        ';' => ";",
                        ',' => ",",
                        '=' => "=",
                        '>' => ">",
                        '!' => "!",
                        '+' => "+",
                        '-' => "-",
                        '*' => "*",
                        '/' => "/",
                        '|' => "|",
                        '^' => "^",
                        _ => {
                            return Err(QueryError::syntax(
                                format!("unexpected character {c:?}"),
                                pos,
                            ))
                        }
                    },
                };
                for _ in 0..p.len() {
                    self.bump();
                }
                Tok::Punct(p)
            }
        };
        Ok(Some(Token { tok, pos }))
    }
}

/// Every token of `text`, in order.
pub fn tokens(text: &str) -> Result<Vec<Token>, QueryError> {
    let mut lx = Lexer {
        chars: text.chars().collect(),
        i: 0,
        line: 1,
        column: 1,
    };
    let mut out = Vec::new();
    while let Some(t) = lx.next()? {
        out.push(t);
    }
    Ok(out)
}

/// Decodes a local name's `%XX` escapes.
pub fn percent_decode(local: &str) -> String {
    let bytes = local.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&local[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
