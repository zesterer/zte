use crate::{lang::LangPack, state::Text};
use std::{
    ops::{Range, RangeInclusive},
    rc::Rc,
};

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
    // matchers: Vec<Regex>,
    matchers: Vec<CompiledRegex>,
    entries: Vec<(TokenKind, Option<Highlighter>)>,
}

impl Highlighter {
    pub fn with(mut self, token: TokenKind, p: impl AsRef<str>) -> Self {
        self.entries.push((token, None));
        self.matchers.push(
            Regex::parser()
                .parse(p.as_ref())
                .unwrap()
                .optimise()
                .compile(),
        );
        self
    }

    pub fn with_child_syntax(
        mut self,
        token: TokenKind,
        p: impl AsRef<str>,
        child: Highlighter,
    ) -> Self {
        self.entries.push((token, Some(child)));
        self.matchers.push(
            Regex::parser()
                .parse(p.as_ref())
                .unwrap()
                .optimise()
                .compile(),
        );
        self
    }

    pub fn highlight_str(&self, text: &str, range: Range<usize>) -> (Vec<Token>, usize) {
        let mut tokens = Vec::new();
        let mut i = range.start;
        loop {
            i = if i >= range.end.min(text.len()) {
                break;
            } else if let Some((idx, n)) = self
                .matchers
                .iter()
                .enumerate()
                .find_map(|(idx, r)| Some((idx, r.matches(text, i)?)))
            {
                let (kind, child_highlighter) = &self.entries[idx];
                tokens.push(Token {
                    kind: *kind,
                    range: i..n,
                    children: if let Some(child_highlighter) = child_highlighter {
                        child_highlighter.highlight_str(text, i..n).0
                    } else {
                        Vec::new()
                    },
                });
                n.max(1)
            } else {
                i + text[i..].chars().next().unwrap().len_utf8()
            };
        }
        (tokens, i)
    }
}

impl LangPack {
    fn delims(&self, text: &Text) -> Vec<DelimTree> {
        let mut chars = text.char_indices();
        let mut top_level = Vec::new();
        let mut open = Vec::new();
        loop {
            let Some((c, i)) = chars.next() else {
                break top_level;
            };
            if let Some((_, e)) = self.delims.iter().find(|(s, _)| *s == c) {
                open.push((i, e, Vec::new()));
            } else if self.delims.iter().any(|(_, e)| *e == c)
                && let Some(&(broken_start, e, _)) = open.last()
                && (c == *e || {
                    let end_indent = text.indent_of_line(text.to_coord(i)[1]);
                    let start_indent = text.indent_of_line(text.to_coord(broken_start)[1]);
                    start_indent == end_indent
                })
                && let Some((start, e, children)) = open.pop()
            {
                let tree = DelimTree {
                    span: start..i + 1,
                    children,
                };
                if let Some((_, _, prev)) = open.last_mut() {
                    prev.push(tree);
                    if c != *e {
                        continue;
                    }
                } else {
                    top_level.push(tree);
                }
            }
        }
    }

    pub fn highlight(&self, text: &Text) -> Highlights {
        Highlights {
            tokens: TokenCache::default(),
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
    pub tokens: TokenCache,
    pub delims: Vec<DelimTree>,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub range: Range<usize>,
    children: Vec<Token>,
}

impl Highlights {
    pub fn get_at(&mut self, highlighter: &Highlighter, text: &Text, pos: usize) -> Option<&Token> {
        self.tokens.get_at(highlighter, text, pos)
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

    pub fn sync(&mut self, lang: &LangPack, text: &Text) {
        self.delims = lang.delims(text);
    }

    pub fn damage_insert(&mut self, r: Range<usize>) {
        self.tokens.damage_from(r.start);
    }

    pub fn damage_remove(&mut self, r: Range<usize>) {
        self.tokens.damage_from(r.start);
    }

    pub fn damage_all(&mut self) {
        self.tokens = TokenCache::default();
    }
}

#[derive(Default)]
pub struct TokenCache {
    blocks: Vec<TokenBlock>,
    total_len: usize,
    flat_text: String,
}

// How many characters should be highlighted in one go before we decide that incremental updates are better?
const HIGHLIGHT_BLOCK: usize = 1024;

impl TokenCache {
    // TODO: A smarter approach would be to damage blocks that intersect the damage area, then offset everything
    // accordingly, then keep replacing blocks until we find one that finished at the same place an existing
    // one started.
    pub fn damage_from(&mut self, pos: usize) {
        let idx = self
            .blocks
            .binary_search_by_key(&pos, |b| b.start)
            .unwrap_or_else(|p| p)
            .saturating_sub(1);
        if let Some(b) = self.blocks.get(idx) {
            self.total_len = b.start;
            self.blocks.truncate(idx);
        }
    }

    pub fn get_at(&mut self, highlighter: &Highlighter, text: &Text, pos: usize) -> Option<&Token> {
        // OOB
        if pos >= text.len() {
            return None;
        }

        // Grow the flat string until it covers the area we need
        self.flat_text.clear();
        for s in text.slice(..).chunks() {
            self.flat_text += s;
        }

        // Grow the highlight cache until it covers `pos`
        while self.total_len <= pos {
            let (tokens, end) = highlighter.highlight_str(
                &self.flat_text,
                self.total_len..self.total_len + HIGHLIGHT_BLOCK,
            );
            self.blocks.push(TokenBlock {
                start: self.total_len,
                tokens,
            });
            assert!(
                end > self.total_len,
                "{}, {}, {}",
                text.len(),
                self.total_len,
                end
            );
            self.total_len = end;
        }

        let idx = self
            .blocks
            .binary_search_by_key(&pos, |b| b.start)
            .unwrap_or_else(|p| p.saturating_sub(1));
        let block = self.blocks.get(idx).expect("no block?");

        TokenBlock::get_at_inner(&block.tokens, pos)
    }
}

struct TokenBlock {
    start: usize,
    tokens: Vec<Token>,
}

impl TokenBlock {
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
}

#[derive(Clone, Debug)]
pub enum Regex {
    Whitespace,
    WordBoundary,
    LineStart,
    LineEnd,
    LastDelim,
    Range(RangeInclusive<char>),
    Char(char),
    CharGroup(Vec<char>),
    CharSet(Vec<RangeInclusive<char>>),
    Set(Vec<Self>),
    NegSet(Vec<Self>),
    Group(Vec<Self>),
    Maybe(Box<Self>),
    Repeat(Box<Self>),
    AtLeastOnce(Box<Self>),
    // (delimiter, x) - parse a pattern, then refer to the substring later in x with `~`
    Delim(Box<Self>, Box<Self>),
    Rewind(Box<Self>),
}

pub struct CompiledRegex(Rc<dyn Fn(&mut State) -> Option<()>>);

impl CompiledRegex {
    pub fn matches(&self, text: &str, at: usize) -> Option<usize> {
        let mut s = State {
            text,
            pos: at,
            delim: None,
        };
        (self.0)(&mut s).map(|_| s.pos)
    }
}

pub struct CompiledRegex2 {
    tables: Vec<[(u16, u16); 256]>,
    entry: usize,
}

impl Regex {
    pub fn optimise(mut self) -> Self {
        match self {
            Self::Group(ref mut xs) => {
                if xs.len() == 1 {
                    xs.remove(0).optimise()
                } else if let Some(xs) = xs
                    .iter()
                    .map(|r| match r {
                        Regex::Char(c) => Some(*c),
                        _ => None,
                    })
                    .collect::<Option<_>>()
                {
                    Self::CharGroup(xs)
                } else {
                    Self::Group(
                        core::mem::take(xs)
                            .into_iter()
                            .map(Self::optimise)
                            .collect(),
                    )
                }
            }
            Self::Set(ref mut xs) => {
                if xs.len() == 1 {
                    xs.remove(0).optimise()
                } else if let Some(xs) = xs
                    .iter()
                    .map(|r| match r {
                        Regex::Char(c) => Some(*c..=*c),
                        Regex::Range(r) => Some(r.clone()),
                        _ => None,
                    })
                    .collect::<Option<_>>()
                {
                    Self::CharSet(xs)
                } else {
                    Self::Set(
                        core::mem::take(xs)
                            .into_iter()
                            .map(Self::optimise)
                            .collect(),
                    )
                }
            }

            // Rec
            Self::NegSet(xs) => Self::NegSet(xs.into_iter().map(Self::optimise).collect()),
            Self::Maybe(x) => Self::Maybe(Box::new(x.optimise())),
            Self::Repeat(x) => Self::Repeat(Box::new(x.optimise())),
            Self::AtLeastOnce(x) => Self::AtLeastOnce(Box::new(x.optimise())),
            Self::Delim(x, y) => Self::Delim(Box::new(x.optimise()), Box::new(y.optimise())),
            Self::Rewind(x) => Self::Rewind(Box::new(x.optimise())),

            // Identity
            Self::Whitespace
            | Self::WordBoundary
            | Self::LineStart
            | Self::LineEnd
            | Self::LastDelim
            | Self::Range(_)
            | Self::Char(_)
            | Self::CharGroup(_)
            | Self::CharSet(_) => self,
        }
    }

    pub fn compile(self) -> CompiledRegex {
        CompiledRegex(match self {
            Self::Whitespace => Rc::new(move |state| state.go(&Self::Whitespace)),
            Self::WordBoundary => Rc::new(move |state| state.go(&Self::WordBoundary)),
            Self::LineStart => Rc::new(move |state| state.go(&Self::LineStart)),
            Self::LineEnd => Rc::new(move |state| state.go(&Self::LineEnd)),
            Self::LastDelim => Rc::new(move |state| state.go(&Self::LastDelim)),
            Self::Range(r) => Rc::new(move |state| state.go(&Self::Range(r.clone()))),
            Self::Char(c) => Rc::new(move |state| state.go(&Self::Char(c))),
            Self::CharGroup(xs) => Rc::new(move |state| {
                for x in &xs {
                    state.skip_if(|c| c == *x)?;
                }
                Some(())
            }),
            Self::CharSet(xs) => Rc::new(move |state| {
                for x in &xs {
                    if state.skip_if(|c| x.contains(&c)).is_some() {
                        return Some(());
                    }
                }
                None
            }),
            Self::Set(xs) => {
                let xs = xs.into_iter().map(Self::compile).collect::<Vec<_>>();
                Rc::new(move |state| xs.iter().find_map(|x| state.attempt_f(&*x.0)))
            }
            Self::NegSet(xs) => {
                let xs = xs.into_iter().map(Self::compile).collect::<Vec<_>>();
                Rc::new(move |state| {
                    if xs.iter().all(|x| state.attempt_f(&*x.0).is_none()) {
                        state.skip_if(|_| true)?;
                        Some(())
                    } else {
                        None
                    }
                })
            }
            Self::Group(xs) => {
                // Attempt at tail call
                // let mut xs = xs.into_iter().rev().map(Self::compile);
                // match xs.next() {
                //     Some(last) => xs.fold(last.0, |end, x| Rc::new(move |state: &mut State| { state.go_f(&*x.0)?; (&end)(state) })),
                //     None => Rc::new(|_| Some(())),
                // }
                let xs = xs.into_iter().map(Self::compile).collect::<Vec<_>>();
                Rc::new(move |state| {
                    for x in &xs {
                        state.go_f(&*x.0)?;
                    }
                    Some(())
                })
            }
            Self::Maybe(x) => {
                let x = x.compile();
                Rc::new(move |state| {
                    let _ = state.attempt_f(&*x.0);
                    Some(())
                })
            }
            Self::Repeat(x) => {
                let x = x.compile();
                Rc::new(move |state| {
                    loop {
                        let pos = state.pos;
                        if state.attempt_f(&*x.0).is_none() {
                            break Some(());
                        }
                        assert!(pos != state.pos);
                    }
                })
            }
            Self::AtLeastOnce(x) => {
                let x = x.compile();
                Rc::new(move |state| {
                    state.attempt_f(&*x.0)?;
                    loop {
                        if state.attempt_f(&*x.0).is_none() {
                            break Some(());
                        }
                    }
                })
            }
            Self::Delim(d, r) => {
                let d = d.compile();
                let r = r.compile();
                Rc::new(move |state| {
                    let old_pos = state.pos;
                    state.go_f(&*d.0)?;
                    let old_delim = state.delim.replace(&state.text[old_pos..state.pos]);
                    let res = state.go_f(&*r.0);
                    state.delim = old_delim;
                    res
                })
            }
            Self::Rewind(r) => {
                let r = r.compile();
                Rc::new(move |state| {
                    let old_pos = state.pos;
                    let res = state.go_f(&*r.0);
                    if res.is_some() {
                        state.pos = old_pos;
                    }
                    res
                })
            }
        })
    }

    fn compile2_inner(self, out: &mut CompiledRegex2, ok: (u16, u16), fail: (u16, u16)) -> usize {
        match self {
            Self::Whitespace => out.populate(|c| c.is_ascii_whitespace(), ok, fail),
            Self::CharSet(xs) => {
                out.populate(|c| xs.iter().any(|x| x.contains(&(c as char))), ok, fail)
            }
            Self::Repeat(x) => {
                let old_tables = out.tables.len();
                let addr =
                    x.compile2_inner(out, (CompiledRegex2::CONTINUE, CompiledRegex2::FIXUP), ok);
                for t in &mut out.tables[old_tables..] {
                    for (_, a) in t {
                        *a = match *a {
                            CompiledRegex2::FIXUP => addr as u16,
                            a => a,
                        };
                    }
                }
                addr
            }
            x => todo!("{x:?}"),
        }
    }

    pub fn compile2(self) -> CompiledRegex2 {
        let mut out = CompiledRegex2 {
            tables: Vec::new(),
            entry: 0,
        };
        out.entry =
            self.compile2_inner(&mut out, (CompiledRegex2::OK, 0), (CompiledRegex2::FAIL, 0));
        out
    }
}

impl CompiledRegex2 {
    const OK: u16 = 0;
    const FAIL: u16 = 1;
    const PUSH: u16 = 2;
    const POP: u16 = 3;
    const CONTINUE: u16 = 4;
    const FIXUP: u16 = 0xFFFF;

    fn populate(&mut self, f: impl Fn(u8) -> bool, ok: (u16, u16), fail: (u16, u16)) -> usize {
        let idx = self.tables.len();
        self.tables
            .push(core::array::from_fn(|x| if f(x as u8) { ok } else { fail }));
        idx
    }

    pub fn matches(&self, s: &str, mut at: usize) -> Option<usize> {
        let mut next = self.entry;
        let mut stack = Vec::new();
        loop {
            let b = s.as_bytes().get(at).copied().unwrap_or(0);
            let (action, goto) = self.tables[next as usize][b as usize];
            next = goto as usize;
            match action {
                Self::OK => return Some(at),
                Self::FAIL => return None,
                Self::PUSH => stack.push(at),
                Self::POP => at = stack.pop().unwrap(),
                _ => {}
            }
            at += 1;
        }
        None
    }
}

struct State<'a> {
    text: &'a str,
    pos: usize,
    delim: Option<&'a str>,
}

impl State<'_> {
    fn peek(&mut self) -> Option<char> {
        self.text[self.pos..].chars().next()
    }

    fn prev(&self) -> Option<char> {
        self.text[..self.pos].chars().next_back()
    }

    fn skip_if(&mut self, f: impl FnOnce(char) -> bool) -> Option<()> {
        let c = self.peek().filter(|c| f(*c))?;
        self.pos += c.len_utf8();
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

    fn attempt_f(&mut self, f: &(impl Fn(&mut State) -> Option<()> + ?Sized)) -> Option<()> {
        let old_pos = self.pos;
        if f(self).is_some() {
            Some(())
        } else {
            self.pos = old_pos;
            None
        }
    }

    fn go_f(&mut self, f: &(impl Fn(&mut State) -> Option<()> + ?Sized)) -> Option<()> {
        f(self)
    }

    #[inline(always)]
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
                let delim = self.delim.expect("no delimiter captured");
                for d in delim.chars() {
                    self.skip_if(|c| c == d)?;
                }
                Some(())
            }
            Regex::Char(x) => self.skip_if(|c| c == *x),
            Regex::CharGroup(xs) => {
                for x in xs {
                    self.skip_if(|c| c == *x)?;
                }
                Some(())
            }
            Regex::CharSet(xs) => {
                for x in xs {
                    if self.skip_if(|c| x.contains(&c)).is_some() {
                        return Some(());
                    }
                }
                None
            }
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
            Regex::Range(r) => self.skip_if(|c| r.contains(&&c)),
            Regex::Maybe(x) => {
                let _ = self.attempt(x);
                Some(())
            }
            Regex::Repeat(x) => loop {
                if self.attempt(x).is_none() {
                    break Some(());
                }
            },
            Regex::AtLeastOnce(x) => {
                self.go(x)?;
                loop {
                    if self.attempt(x).is_none() {
                        break Some(());
                    }
                }
            }
            Regex::Delim(d, r) => {
                let old_pos = self.pos;
                self.go(d)?;
                let old_delim = self.delim.replace(&self.text[old_pos..self.pos]);
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
    pub fn matches(&self, text: &str, at: usize) -> Option<usize> {
        let mut s = State {
            text,
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

impl Regex {
    pub fn parser<'a>() -> impl Parser<'a, &'a str, Self, extra::Err<Rich<'a, char>>> {
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
                .map(|(a, b)| Self::Range(a..=b));

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
                postfix(1, just('*'), |r, _, _| Self::Repeat(Box::new(r))),
                postfix(1, just('+'), |r, _, _| Self::AtLeastOnce(Box::new(r))),
                postfix(1, just('?'), |r, _, _| Self::Maybe(Box::new(r))),
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
