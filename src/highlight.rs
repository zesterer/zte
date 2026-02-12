use crate::{lang::LangPack, state::Text};
use std::ops::Range;

#[derive(Copy, Clone, Debug, PartialEq)]
#[allow(dead_code)]
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
    /// A macro-specific keyword, usually nested within a macro
    MacroKeyword,
    /// A string literal
    String,
    /// A format element in a string
    FmtString,
    /// Misc special syntax (defined per-language)
    Special,
    /// A program constant or other statically-known name
    Constant,
    /// A function call or some other active operation
    Function,
    /// An active merge conflict
    MergeConflict,
    /// A clickable URL
    Url,
    /// An important warning, such as a TODO comment
    Important,
    /// An underlined title
    Title,
    /// Italic element in a document
    Italic,
    /// Bold element in a document
    Bold,
}

#[derive(Default)]
pub struct Highlighter {
    matchers: Vec<Regex>,
    entries: Vec<(TokenKind, Option<Highlighter>)>,
}

impl Highlighter {
    pub fn with(mut self, token: TokenKind, p: impl AsRef<str>) -> Self {
        self.entries.push((token, None));
        self.matchers
            .push(Regex::parser().parse(p.as_ref()).unwrap());
        self
    }

    pub fn with_child_syntax(
        mut self,
        token: TokenKind,
        p: impl AsRef<str>,
        child: Highlighter,
    ) -> Self {
        self.entries.push((token, Some(child)));
        self.matchers
            .push(Regex::parser().parse(p.as_ref()).unwrap());
        self
    }

    fn highlight_str(&self, s: &[char], range: Range<usize>) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut i = range.start;
        loop {
            i = if let Some((idx, n)) = self
                .matchers
                .iter()
                .enumerate()
                .find_map(|(idx, r)| Some((idx, r.matches(s, i)?)))
            {
                let (kind, child_highlighter) = &self.entries[idx];
                tokens.push(Token {
                    kind: *kind,
                    range: i..n,
                    children: if let Some(child_highlighter) = child_highlighter {
                        child_highlighter.highlight_str(s, i..n)
                    } else {
                        Vec::new()
                    },
                });
                n
            } else if i < range.end.min(s.len()) {
                i + 1
            } else {
                break;
            };
        }
        tokens
    }
}

impl LangPack {
    fn delims(&self, text: &Text) -> Vec<DelimTree> {
        let mut i = 0;
        let mut top_level = Vec::new();
        let mut open = Vec::new();
        loop {
            let c = text.chars().get(i);
            if let Some(c) = c
                && let Some((_, e)) = self.delims.iter().find(|(s, _)| s == c)
            {
                open.push((i, e, Vec::new()));
            } else if (self.delims.iter().any(|(_, e)| Some(e) == c) || c.is_none())
                && let Some(&(broken_start, e, ref prev)) = open.last()
                && ((c == Some(e) && prev.is_empty()) || {
                    let end_indent = text.indent_of_line(text.to_coord(i)[1]);
                    let start_indent = text.indent_of_line(text.to_coord(broken_start)[1]);
                    end_indent
                        .strip_prefix(start_indent)
                        .map_or(true, |s| s.is_empty())
                })
                && let Some((start, e, children)) = open.pop()
            {
                let tree = DelimTree {
                    span: start..i + 1,
                    children,
                };
                if let Some((_, _, prev)) = open.last_mut() {
                    prev.push(tree);
                    if c != Some(e) {
                        continue;
                    }
                } else {
                    top_level.push(tree);
                }
            } else if c.is_none() {
                break top_level;
            }
            i += 1;
        }
    }

    pub fn highlight(&self, text: &Text) -> Highlights {
        let tokens = self
            .highlighter
            .highlight_str(text.chars(), 0..text.chars().len());
        Highlights {
            tokens,
            delims: self.delims(text),
        }
    }
}

pub struct DelimTree {
    span: Range<usize>,
    children: Vec<Self>,
}

#[derive(Default)]
pub struct Highlights {
    tokens: Vec<Token>,
    delims: Vec<DelimTree>,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub range: Range<usize>,
    pub children: Vec<Token>,
}

impl Highlights {
    fn get_at_inner(tokens: &[Token], pos: usize) -> Option<&Token> {
        let idx = tokens
            .binary_search_by_key(&pos, |tok| tok.range.start)
            .unwrap_or_else(|p| p.saturating_sub(1));
        let tok = tokens.get(idx)?;
        if tok.range.contains(&pos) {
            // Check child tokens too
            let tok = if !tok.children.is_empty()
                && let Some(tok) = Self::get_at_inner(&tok.children, pos)
            {
                tok
            } else {
                tok
            };
            Some(tok)
        } else {
            None
        }
    }

    pub fn get_at(&self, pos: usize) -> Option<&Token> {
        Self::get_at_inner(&self.tokens, pos)
    }

    fn get_delim_at_inner(
        &self,
        delim: &DelimTree,
        f: &mut impl FnMut(Range<usize>) -> bool,
    ) -> Option<(usize, Range<usize>)> {
        if f(delim.span.clone()) {
            for c in &delim.children {
                if let Some((depth, range)) = self.get_delim_at_inner(c, f) {
                    return Some((depth + 1, range));
                }
            }
            Some((0, delim.span.clone()))
        } else {
            None
        }
    }

    pub fn get_delim_at(
        &self,
        mut f: impl FnMut(Range<usize>) -> bool,
    ) -> Option<(usize, Range<usize>)> {
        for d in &self.delims {
            if let Some((depth, range)) = self.get_delim_at_inner(d, &mut f) {
                return Some((depth, range));
            }
        }
        None
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
    // (delimiter, x) - parse a pattern, then refer to the substring later in x with `~`
    Delim(Box<Self>, Box<Self>),
    Rewind(Box<Self>),
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
            Regex::Rewind(r) => {
                let old_pos = self.pos;
                let res = self.go(r);
                if res.is_some() {
                    self.pos = old_pos;
                }
                res
            }
        }
    }
}

impl Regex {
    fn matches(&self, s: &[char], at: usize) -> Option<usize> {
        let mut s = State {
            s,
            pos: at,
            delim: None,
        };
        s.go(self).map(|_| s.pos)
    }
}

use chumsky::{
    pratt::{infix, left, postfix},
    prelude::*,
};

#[test]
fn regex() {
    let reg = Regex::parser().parse(r"\b[0-9][A-Za-z0-9_\.]*\b").unwrap();
    dbg!(&reg);
    assert!(reg.matches(&"5".chars().collect::<Vec<_>>()).is_some());
    panic!("done");
}

impl Regex {
    fn parser<'a>() -> impl Parser<'a, &'a str, Self, extra::Err<Rich<'a, char>>> {
        recursive(|regex| {
            let metachars = r"{}[]()^$.|*+-?\/@~%";
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
                just("\\b").to(Self::WordBoundary),
                just("^").to(Self::LineStart),
                just("$").to(Self::LineEnd),
                just("~").to(Self::LastDelim),
                // Classes
                just("[[:space:]]").map(|_| Self::Whitespace),
                range,
                char_.map(Self::Char),
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
                // Non-standard: match the lhs, then rewind the input (i.e: as if it had never been parsed).
                // Most useful at the end of tokens for context-sensitivie behaviour. For example, differentiating idents and function calls
                postfix(1, just('%'), |r, _, _| Self::Rewind(Box::new(r))),
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
