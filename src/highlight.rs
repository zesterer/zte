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

#[derive(Default)]
pub struct Highlighter {
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
        self.with_many([(token, p)])
    }

    pub fn with_many<P: AsRef<str>>(
        mut self,
        patterns: impl IntoIterator<Item = (TokenKind, P)>,
    ) -> Self {
        for (token, p) in patterns {
            self.entries.push(token);
            self.matchers
                .push(Regex::parser().parse(p.as_ref()).unwrap());
        }
        self
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

    pub fn highlight(&self, s: &[char]) -> Highlights {
        let tokens = self.highlight_str(s);
        Highlights { tokens }
    }
}

#[derive(Default)]
pub struct Highlights {
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
