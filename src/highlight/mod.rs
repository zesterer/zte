use std::{ops::Range, path::Path};

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TokenKind {
    Whitespace,
    Ident,
    Keyword,
    Number,
    Type,
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

        Self {
            entries,
            /*regex: meta::Regex::new_many(&patterns).unwrap(),*/ matchers,
        }
    }

    pub fn from_file_name(file_name: &Path) -> Option<Self> {
        match file_name.extension()?.to_str()? {
            "rs" => Some(Self::rust()),
            _ => None,
        }
    }

    pub fn rust() -> Self {
        Self::new_from_regex([
            (
                TokenKind::Keyword,
                r"\b[(pub)(enum)(let)(self)(Self)(fn)(impl)(struct)(use)(if)(while)(for)(loop)(mod)]\b",
            ),
            (TokenKind::Ident, r"[a-z_][A-Za-z0-9_]*"),
            (TokenKind::Type, r"[A-Z_][A-Za-z0-9_]*"),
            (TokenKind::Number, r"[0-9][A-Za-z0-9_]*"),
        ])
    }

    fn highlight_str(&self, mut s: &str) -> Vec<(Range<usize>, TokenKind)> {
        let mut tokens = Vec::new();
        let mut i = 0;
        loop {
            let n = if let Some((idx, n)) = self
                .matchers
                .iter()
                .enumerate()
                .find_map(|(i, r)| Some((i, r.matches(s)?)))
            {
                tokens.push((i..i + n, self.entries[idx]));
                n
            } else if let Some((n, _)) = s.char_indices().nth(1) {
                n
            } else {
                break;
            };
            i += n;
            s = &s[n..];
        }
        tokens
    }

    pub fn highlight(self, s: &str) -> Highlights {
        let tokens = self.highlight_str(s);
        Highlights {
            highlighter: self,
            tokens,
        }
    }
}

pub struct Highlights {
    pub highlighter: Highlighter,
    tokens: Vec<(Range<usize>, TokenKind)>,
}

impl Highlights {
    pub fn insert(&mut self, at: usize, s: &str) {}

    pub fn get_at(&self, pos: usize) -> Option<TokenKind> {
        let idx = self.tokens
            .binary_search_by_key(&pos, |(r, _)| r.start)
            // .ok()?
            .unwrap_or_else(|p| p.saturating_sub(1))
            // .saturating_sub(1)
        ;
        let (r, tok) = self.tokens.get(idx)?;
        if r.contains(&pos) { Some(*tok) } else { None }
    }
}

#[derive(Clone, Debug)]
pub enum Regex {
    Whitespace,
    WordBoundary,
    Range(char, char),
    Char(char),
    Set(Vec<Self>),
    Group(Vec<Self>),
    // (at_least, _)
    Many(usize, Box<Self>),
}

struct State<'a> {
    s: &'a str,
    pos: usize,
}

impl State<'_> {
    fn peek(&self) -> Option<char> {
        self.s[self.pos..].chars().next()
    }

    fn prev(&self) -> Option<char> {
        self.s[..self.pos].chars().rev().next()
    }

    fn skip(&mut self) {
        if let Some(c) = self.peek() {
            self.pos += c.len_utf8();
        }
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
            Regex::Char(c) => {
                if self.peek()? == *c {
                    self.skip();
                    Some(())
                } else {
                    None
                }
            }
            Regex::Whitespace => {
                let mut once = false;
                while let Some(c) = self.peek() {
                    if c.is_ascii_whitespace() {
                        self.skip();
                        once = true;
                    } else {
                        break;
                    }
                }
                once.then_some(())
            }
            Regex::Set(xs) => xs.iter().find_map(|x| self.attempt(x)),
            Regex::Group(xs) => {
                for x in xs {
                    self.go(x)?;
                }
                Some(())
            }
            Regex::Range(a, b) => {
                if (a..=b).contains(&&self.peek()?) {
                    self.skip();
                    Some(())
                } else {
                    None
                }
            }
            Regex::Many(at_least, x) => {
                let mut times = 0;
                loop {
                    if self.attempt(x).is_none() {
                        break;
                    }
                    times += 1;
                }

                if times >= *at_least { Some(()) } else { None }
            }
            r => todo!("{r:?}"),
        }
    }
}

impl Regex {
    fn matches(&self, s: &str) -> Option<usize> {
        let mut s = State { s, pos: 0 };
        s.go(self).map(|_| s.pos)
    }
}

use chumsky::{pratt::postfix, prelude::*};

impl Regex {
    fn parser<'a>() -> impl Parser<'a, &'a str, Self, extra::Err<Rich<'a, char>>> {
        recursive(|regex| {
            let char_ = any().filter(|c: &char| c.is_alphanumeric() || *c == '_');

            let range = char_
                .then_ignore(just('-'))
                .then(char_)
                .map(|(a, b)| Self::Range(a, b));

            let atom = choice((
                range,
                char_.map(Self::Char),
                just("\\b").to(Self::WordBoundary),
                // Classes
                just("[[:space:]]").map(|_| Self::Whitespace),
                regex
                    .clone()
                    .repeated()
                    .collect()
                    .delimited_by(just('['), just(']'))
                    .map(Regex::Set),
                regex
                    .clone()
                    .repeated()
                    .collect()
                    .delimited_by(just('('), just(')'))
                    .map(Regex::Group),
            ));

            atom.pratt((
                postfix(0, just('*'), |r, _, _| Self::Many(0, Box::new(r))),
                postfix(0, just('+'), |r, _, _| Self::Many(1, Box::new(r))),
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
