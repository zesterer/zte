use std::{ops::Range, path::Path};

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TokenKind {
    /// Non-structural whitespace
    Whitespace,
    /// Identifiers and names
    Ident,
    /// Syntax keywords
    Keyword,
    /// Numeric literals
    Number,
    /// Types or type definitions
    Type,
    /// Comments, which have no effect on the code
    Comment,
    /// Documentation or doc comments
    Doc,
    /// Operators that perform work on operands
    Operator,
    /// Structural tokens (parentheses, braces, brackets, etc.)
    Delimiter,
    /// A field or method of another value (i.e: a named thing not present in the current namespace)
    Property,
    /// A special attribute or decorator attached to some other code
    Attribute,
    /// A macro, that transforms the code in some manner
    Macro,
    /// A string literal
    String,
    /// Misc special syntax (defined per-language)
    Special,
    /// A program constant or other statically-known name
    Constant,
}

pub struct Highlighter {
    // regex: meta::Regex,
    matchers: Vec<Regex>,
    entries: Vec<TokenKind>,
}

impl Highlighter {
    pub fn new_from_regex<P: AsRef<str>>(
        patterns: impl IntoIterator<Item = (TokenKind, P)>,
    ) -> Self {
        let (entries, patterns): (_, Vec<_>) = patterns.into_iter().unzip();

        let matchers = patterns
            .iter()
            .map(|p| Regex::parser().parse(p.as_ref()).unwrap())
            .collect();

        Self { entries, matchers }
    }

    pub fn with(mut self, token: TokenKind, p: impl AsRef<str>) -> Self {
        self.entries.push(token);
        self.matchers
            .push(Regex::parser().parse(p.as_ref()).unwrap());
        self
    }

    pub fn from_file_name(file_name: &Path) -> Option<Self> {
        match file_name.extension()?.to_str()? {
            "rs" => Some(Self::rust()),
            "md" => Some(Self::markdown()),
            "toml" => Some(Self::toml()),
            "c" | "h" | "cpp" | "hpp" | "cxx" | "js" | "ts" | "go" => Some(Self::generic_clike()),
            "glsl" | "vert" | "frag" => Some(Self::glsl()),
            _ => None,
        }
    }

    pub fn markdown() -> Self {
        Self::new_from_regex([
            // Links
            (TokenKind::String, r"\[[^\]]*\](\([^\)]*\))?"),
            // Header
            (TokenKind::Doc, r"^#+[[:space:]][^$]*$"),
            // List item
            (TokenKind::Operator, r"^[[:space:]]?[\-([0-9]+[\)\.])]"),
            // Bold
            (TokenKind::Property, r"\*\*[^(\*\*)]*\*\*"),
            // Italics
            (TokenKind::Attribute, r"\*[^\*]*\*"),
            // Code block
            (TokenKind::Operator, r"^```[^(^```)]*^```"),
            // Inline code
            (TokenKind::Constant, r"`[^`$]*[`$]"),
            // HTML
            (TokenKind::Special, r"<[^<>]*>"),
        ])
    }

    pub fn rust() -> Self {
        Self::new_from_regex([
            // Both kinds of comments match multiple lines
            (
                TokenKind::Doc,
                r"\/\/[\/!][^\n]*$(\n[[:space:]]\/\/[\/!][^\n]*$)*",
            ),
            (TokenKind::Comment, r"\/\/[^$]*$(\n[[:space:]]\/\/[^$]*$)*"),
            // Multi-line comment
            (TokenKind::Comment, r"\/\*[^(\*\/)]*\*\/"),
            (
                TokenKind::Keyword,
                r"\b[(pub)(enum)(let)(self)(Self)(fn)(impl)(struct)(use)(if)(while)(for)(in)(loop)(mod)(match)(else)(break)(continue)(trait)(const)(static)(type)(mut)(as)(crate)(extern)(move)(ref)(return)(super)(unsafe)(use)(where)(async)(dyn)(try)(gen)(macro_rules)(union)(raw)]\b",
            ),
            (TokenKind::Constant, r"\b[(true)(false)]\b"),
            // Flow-control operators count as keywords
            (TokenKind::Keyword, r"\.await\b"),
            // Macro invocations: println!
            (TokenKind::Macro, r"\b[A-Za-z_][A-Za-z0-9_]*!"),
            // Meta-variables
            (TokenKind::Macro, r"\$[A-Za-z_][A-Za-z0-9_]*\b"),
            (TokenKind::Constant, r"\b[A-Z][A-Z0-9_]+\b"),
            (TokenKind::Type, r"\b[A-Z][A-Za-z0-9_]*\b"),
            // Primitives
            (
                TokenKind::Type,
                r"\b[(u8)(u16)(u32)(u64)(u128)(i8)(i16)(i32)(i64)(i128)(usize)(isize)(bool)(str)(char)(f16)(f32)(f64)(f128)]\b",
            ),
            // "foo" or b"foo" or r#"foo"#
            (TokenKind::String, r#"b?r?(#*)@("[(\\")[^("~)]]*("~))"#),
            // Characters
            (
                TokenKind::String,
                r#"b?'[(\\[nrt\\0(x[0-7A-Za-z][0-7A-Za-z])])[^']]'"#,
            ),
            (
                TokenKind::Operator,
                r"[(&(mut)?)(\?)(\+=?)(\-=?)(\*=?)(\/=?)(%=?)(!=?)(==?)(&&?=?)(\|\|?=?)(<<?=?)(>>?=?)(\.\.[\.=]?)\\\~\^:;,\@(=>?)]",
            ),
            // Fields and methods: a.foo
            (TokenKind::Property, r"\.[a-z_][A-Za-z0-9_]*"),
            // Paths: std::foo::bar
            (TokenKind::Property, r"[A-Za-z_][A-Za-z0-9_]*::"),
            // Lifetimes
            (TokenKind::Special, r"'[a-z_][A-Za-z0-9_]*\b"),
            (TokenKind::Ident, r"\b[a-z_][A-Za-z0-9_]*\b"),
            (TokenKind::Number, r"[0-9][A-Za-z0-9_\.]*"),
            (TokenKind::Delimiter, r"[\{\}\(\)\[\]]"),
            (TokenKind::Macro, r"[\{\}\(\)\[\]]"),
            (TokenKind::Attribute, r"#!?\[[^\]]*\]"),
        ])
    }

    pub fn clike(keyword: &str, r#type: &str, builtin: &str) -> Self {
        Self::new_from_regex([
            // Both kinds of comments match multiple lines
            (
                TokenKind::Doc,
                r"\/\/[\/!][^\n]*$(\n[[:space:]]\/\/[\/!][^\n]*$)*",
            ),
            (TokenKind::Comment, r"\/\/[^$]*$(\n[[:space:]]\/\/[^$]*$)*"),
            // Multi-line comment
            (TokenKind::Comment, r"\/\*[^(\*\/)]*\*\/"),
            (TokenKind::Keyword, keyword),
            (TokenKind::Macro, builtin),
            (TokenKind::Constant, r"\b[(true)(false)]\b"),
            // Flow-control operators count as keywords
            (TokenKind::Keyword, r"\b[(\.await)\?]\b"),
            (TokenKind::Constant, r"\b[A-Z][A-Z0-9_]+\b"),
            (TokenKind::Type, r"\b[A-Z][A-Za-z0-9_]*\b"),
            // Primitives
            (TokenKind::Type, r#type),
            // "foo" or b"foo" or r#"foo"#
            (TokenKind::String, r#"b?r?(#*)@("[(\\")[^("~)]]*("~))"#),
            // Character strings
            (
                TokenKind::String,
                r#"b?'[(\\[nrt\\0(x[0-7A-Za-z][0-7A-Za-z])])[^']]*'"#,
            ),
            (
                TokenKind::Operator,
                r"[(&)(\?)(\+\+)(\-\-)(\+=?)(\-=?)(\*=?)(\/=?)(%=?)(!=?)(==?)(&&?=?)(\|\|?=?)(<<?=?)(>>?=?)(\.\.[\.=]?)\\\~\^:;,\@(=>?)]",
            ),
            // Fields and methods: a.foo
            (TokenKind::Property, r"\.[a-z_][A-Za-z0-9_]*"),
            // Paths: std::foo::bar
            (TokenKind::Property, r"[A-Za-z_][A-Za-z0-9_]*::"),
            (TokenKind::Ident, r"\b[a-z_][A-Za-z0-9_]*\b"),
            (TokenKind::Number, r"[0-9][A-Za-z0-9_\.]*"),
            (TokenKind::Delimiter, r"[\{\}\(\)\[\]]"),
            // Preprocessor
            (TokenKind::Macro, r"^#[^$]*$"),
        ])
    }

    pub fn generic_clike() -> Self {
        Self::clike(
            // keyword
            r"\b[(var)(enum)(let)(this)(fn)(struct)(class)(import)(if)(while)(for)(in)(loop)(else)(break)(continue)(const)(static)(type)(extern)(return)(async)(throw)(catch)(union)(auto)(namespace)(public)(private)(function)(func)]\b",
            // types
            r"\b[(([(unsigned)(signed)][[:space:]])*u?int[0-9]*(_t)?)(float)(double)(bool)(char)(size_t)(void)]\b",
            "[]",
        )
    }

    pub fn glsl() -> Self {
        Self::clike(
            // keyword
            r"\b[(struct)(if)(while)(for)(else)(break)(continue)(const)(return)(layout)(uniform)(set)(binding)(location)(in)]\b",
            // types
            r"\b[(u?int)(float)(double)(bool)(void)([ui]?vec[1-4]*)([ui]?mat[1-4]*)(texture[(2D)(3D)]?(Cube)?)([ui]?sampler[(2D)(3D)]?(Shadow)?)]\b",
            // Builtins
            r"\b[(dot)(cross)(textureSize)(normalize)(texelFetch)(textureProj)(max)(min)(clamp)(reflect)(mix)(distance)(length)(abs)(pow)(sign)(sin)(cos)(tan)(fract)(mod)(round)(step)]\b",
        )
    }

    pub fn toml() -> Self {
        Self::new_from_regex([
            // Header
            (TokenKind::Doc, r#"^\[[^\n\]]*\]$"#),
            // Delimiters
            (TokenKind::Delimiter, r"[\{\}\(\)\[\]]"),
            // Operators
            (TokenKind::Operator, r"[=,]"),
            // Numbers
            (TokenKind::Number, r"[0-9][A-Za-z0-9_\.]*"),
            // Double-quoted strings
            (
                TokenKind::String,
                r#"b?"[(\\[nrt\\0(x[0-7A-Za-z][0-7A-Za-z])])[^"]]*""#,
            ),
            // Single-quoted strings
            (
                TokenKind::String,
                r#"b?'[(\\[nrt\\0(x[0-7A-Za-z][0-7A-Za-z])])[^']]*'"#,
            ),
            // Booleans
            (TokenKind::Constant, r"\b[(true)(false)]\b"),
            // Identifier
            (TokenKind::Ident, r"\b[a-z_][A-Za-z0-9_\-]*\b"),
            // Comments
            (TokenKind::Comment, r"#[^$]*$"),
        ])
    }

    fn highlight_str(&self, mut s: &[char]) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut i = 0;
        loop {
            let n = if let Some((idx, n)) = self
                .matchers
                .iter()
                .enumerate()
                .find_map(|(i, r)| Some((i, r.matches(s)?)))
            {
                tokens.push(Token {
                    kind: self.entries[idx],
                    range: i..i + n,
                });
                n
            } else if !s.is_empty() {
                1
            } else {
                break;
            };
            i += n;
            s = &s[n..];
        }
        tokens
    }

    pub fn highlight(self, s: &[char]) -> Highlights {
        let tokens = self.highlight_str(s);
        Highlights {
            highlighter: self,
            tokens,
        }
    }
}

pub struct Highlights {
    pub highlighter: Highlighter,
    tokens: Vec<Token>,
}

#[derive(Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub range: Range<usize>,
}

impl Highlights {
    pub fn insert(&mut self, at: usize, s: &str) {}

    pub fn get_at(&self, pos: usize) -> Option<&Token> {
        let idx = self.tokens
            .binary_search_by_key(&pos, |tok| tok.range.start)
            // .ok()?
            .unwrap_or_else(|p| p.saturating_sub(1))
            // .saturating_sub(1)
        ;
        let tok = self.tokens.get(idx)?;
        if tok.range.contains(&pos) {
            Some(tok)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug)]
pub enum Regex {
    Whitespace,
    WordBoundary,
    LineStart,
    LineEnd,
    LastDelim,
    Range(char, char),
    Char(char),
    Set(Vec<Self>),
    NegSet(Vec<Self>),
    Group(Vec<Self>),
    // (at_least, at_most, _)
    Many(usize, usize, Box<Self>),
    // (delimiter, x) - delimit x with `delimiter` on either side (used for raw strings)
    Delim(Box<Self>, Box<Self>),
}

struct State<'a> {
    s: &'a [char],
    pos: usize,
    delim: Option<&'a [char]>,
}

impl State<'_> {
    fn peek(&self) -> Option<char> {
        self.s.get(self.pos).copied()
    }

    fn prev(&self) -> Option<char> {
        self.s[..self.pos].last().copied()
        // self.s.get(self.pos.saturating_sub(1)).copied()
    }

    fn skip_if(&mut self, f: impl FnOnce(char) -> bool) -> Option<()> {
        self.peek().filter(|c| f(*c))?;
        self.pos += 1;
        Some(())
    }

    fn attempt(&mut self, r: &Regex) -> Option<()> {
        let old_pos = self.pos;
        if self.go(r).is_some() {
            Some(())
        } else {
            self.pos = old_pos;
            None
        }
    }

    fn go(&mut self, r: &Regex) -> Option<()> {
        match r {
            Regex::WordBoundary => {
                let is_word = |c: char| c.is_alphanumeric() || c == '_';
                (is_word(self.prev().unwrap_or(' ')) != is_word(self.peek().unwrap_or(' ')))
                    .then_some(())
            }
            Regex::LineStart => self.prev().map_or(true, |c| c == '\n').then_some(()),
            Regex::LineEnd => self.peek().map_or(true, |c| c == '\n').then_some(()),
            Regex::LastDelim => {
                if self.s[self.pos..].starts_with(self.delim?) {
                    self.pos += self.delim.unwrap().len();
                    Some(())
                } else {
                    None
                }
            }
            Regex::Char(x) => self.skip_if(|c| c == *x),
            Regex::Whitespace => {
                let mut once = false;
                while self.skip_if(|c| c.is_ascii_whitespace()).is_some() {
                    once = true;
                }
                once.then_some(())
            }
            Regex::NegSet(xs) => {
                if xs.iter().all(|x| self.attempt(x).is_none()) {
                    self.skip_if(|_| true)?;
                    Some(())
                } else {
                    None
                }
            }
            Regex::Set(xs) => xs.iter().find_map(|x| self.attempt(x)),
            Regex::Group(xs) => {
                for x in xs {
                    self.go(x)?;
                }
                Some(())
            }
            Regex::Range(a, b) => self.skip_if(|c| (a..=b).contains(&&c)),
            Regex::Many(at_least, at_most, x) => {
                let mut times = 0;
                loop {
                    let pos = self.pos;
                    if times >= *at_most || self.attempt(x).is_none() {
                        break (times >= *at_least).then_some(());
                    }
                    assert_ne!(pos, self.pos, "{x:?}");
                    times += 1;
                }
            }
            Regex::Delim(d, r) => {
                let old_pos = self.pos;
                self.go(d)?;
                let old_delim = self.delim.replace(&self.s[old_pos..self.pos]);
                let res = self.go(r);
                self.delim = old_delim;
                res
            }
        }
    }
}

impl Regex {
    fn matches(&self, s: &[char]) -> Option<usize> {
        let mut s = State {
            s,
            pos: 0,
            delim: None,
        };
        s.go(self).map(|_| s.pos)
    }
}

use chumsky::{
    pratt::{infix, left, postfix},
    prelude::*,
};

impl Regex {
    fn parser<'a>() -> impl Parser<'a, &'a str, Self, extra::Err<Rich<'a, char>>> {
        recursive(|regex| {
            let metachars = r"{}[]()^$.|*+-?\/@~";
            let char_ = choice((
                none_of(metachars),
                // Escaped meta characters
                just('\\').ignore_then(one_of(metachars)),
                just("\\n").to('\n'),
            ));

            let range = char_
                .then_ignore(just('-'))
                .then(char_)
                .map(|(a, b)| Self::Range(a, b));

            let items = regex.clone().repeated().collect();

            let atom = choice((
                range,
                char_.map(Self::Char),
                just("\\b").to(Self::WordBoundary),
                just("^").to(Self::LineStart),
                just("$").to(Self::LineEnd),
                just("~").to(Self::LastDelim),
                // Classes
                just("[[:space:]]").map(|_| Self::Whitespace),
                items
                    .clone()
                    .delimited_by(just("[^"), just(']'))
                    .map(Regex::NegSet),
                items
                    .clone()
                    .delimited_by(just('['), just(']'))
                    .map(Regex::Set),
                items
                    .clone()
                    .delimited_by(just('('), just(')'))
                    .map(Regex::Group),
            ));

            atom.pratt((
                postfix(1, just('*'), |r, _, _| Self::Many(0, !0, Box::new(r))),
                postfix(1, just('+'), |r, _, _| Self::Many(1, !0, Box::new(r))),
                postfix(1, just('?'), |r, _, _| Self::Many(0, 1, Box::new(r))),
                // Non-standard: `x@y` parses `x` and then `y`. `y` can use `~` to refer to the extra string that was
                // parsed by `x`. This supports nesting and is intended for context-sensitive patterns like Rust raw
                // strings.
                infix(left(0), just('@'), |d, _, r, _| {
                    Self::Delim(Box::new(d), Box::new(r))
                }),
            ))
        })
        .repeated()
        .collect()
        .map(Self::Group)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple() {
        let hl = Highlighter::rust().highlight("pub");
        assert_eq!(hl.tokens, Vec::new());
    }
}
