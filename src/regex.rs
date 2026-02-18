use std::ops::{Range, RangeInclusive};

#[derive(Clone, Debug)]
pub enum Regex {
    Whitespace,
    WordBoundary,
    LineStart,
    LineEnd,
    LastDelim,
    Range(RangeInclusive<char>),
    Char(char),
    String(String),
    StringSet(Vec<String>),
    WordSet(Vec<String>),
    CharSet(String),
    CharRangeSet(Vec<RangeInclusive<char>>),
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

struct State<'a> {
    text: &'a str,
    pos: usize,
    delim: &'a str,
}

impl State<'_> {
    fn before(&self) -> &str {
        // SAFETY: `pos` will always be on a char boundary and <= len
        unsafe { self.text.get_unchecked(..self.pos) }
    }
    fn after(&self) -> &str {
        // SAFETY: `pos` will always be on a char boundary and <= len
        unsafe { self.text.get_unchecked(self.pos..) }
    }

    fn peek(&self) -> Option<char> {
        self.after().chars().next()
    }
    fn peek_byte(&self) -> Option<u8> {
        self.after().as_bytes().first().copied()
    }

    fn prev(&self) -> Option<char> {
        self.before().chars().next_back()
    }
    fn prev_byte(&self) -> Option<u8> {
        self.before().as_bytes().last().copied()
    }

    fn skip_if(&mut self, f: impl FnOnce(char) -> bool) -> Option<()> {
        let c = self.peek().filter(|c| f(*c))?;
        self.pos += c.len_utf8();
        Some(())
    }
    // SAFETY: Must only return `true` if the byte is ASCII
    unsafe fn skip_byte_if(&mut self, f: impl FnOnce(u8) -> bool) -> Option<()> {
        self.peek_byte().filter(|c| f(*c))?;
        self.pos += 1;
        Some(())
    }
    // SAFETY: Must only return `true` if the byte is ASCII
    fn expect_str(&mut self, s: &str) -> Option<()> {
        // SAFETY: `pos` will always be <= len
        if self.after().as_bytes().starts_with(s.as_bytes()) {
            self.pos += s.len();
            Some(())
        } else {
            None
        }
    }

    fn attempt(&mut self, f: impl Fn(&mut State) -> Option<()>) -> Option<()> {
        let old_pos = self.pos;
        if f(self).is_some() {
            Some(())
        } else {
            self.pos = old_pos;
            None
        }
    }

    fn expect_word_boundary(&mut self) -> Option<()> {
        let is_word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
        (is_word(self.prev_byte().unwrap_or(b' ')) != is_word(self.peek_byte().unwrap_or(b' ')))
            .then_some(())
    }
}

pub struct CompiledPattern {
    regex: FastFn,
    prefix: String,
}

impl CompiledPattern {
    pub fn create(regex: &str) -> Self {
        Regex::parser().parse(regex).unwrap().compile()
    }

    pub fn find_nonoverlapping_matches(&self, text: &str) -> impl Iterator<Item = Range<usize>> {
        let mut at = 0;
        core::iter::from_fn(move || {
            loop {
                if let Some(left) = text.get(at..)
                    && let Some(c) = left.chars().next()
                {
                    let start = at;
                    // Fast path
                    if left.starts_with(&self.prefix)
                    // Slow path
                    && let Some(end) = self.matches(text, at)
                    {
                        at = end;
                        break Some(start..end);
                    } else {
                        at += c.len_utf8();
                    }
                } else {
                    break None;
                }
            }
        })
    }

    pub fn matches(&self, text: &str, at: usize) -> Option<usize> {
        assert!(at <= text.len());
        let mut s = State {
            text,
            pos: at,
            delim: "",
        };
        self.regex.call(&mut s).map(|_| s.pos)
    }
}

pub struct FastFn {
    invoke: unsafe fn(*mut (), &mut State) -> Option<()>,
    data: *mut (),
    drop: unsafe fn(*mut ()),
}

impl FastFn {
    fn new<F: Fn(&mut State) -> Option<()> + 'static>(f: F) -> Self {
        unsafe fn invoke<F: Fn(&mut State) -> Option<()> + 'static>(
            data: *mut (),
            state: &mut State,
        ) -> Option<()> {
            let f = unsafe { &*data.cast::<F>() };
            f(state)
        }
        unsafe fn do_drop<F: Fn(&mut State) -> Option<()> + 'static>(data: *mut ()) {
            drop(unsafe { Box::from_raw(data.cast::<F>()) })
        }
        FastFn {
            invoke: invoke::<F>,
            data: Box::into_raw(f.into()).cast(),
            drop: do_drop::<F>,
        }
    }

    fn call(&self, state: &mut State) -> Option<()> {
        unsafe { (self.invoke)(self.data, state) }
    }
}

impl Drop for FastFn {
    fn drop(&mut self) {
        unsafe { (self.drop)(self.data) }
    }
}

impl Regex {
    pub fn optimise(mut self) -> Self {
        match self {
            Self::Group(ref mut xs) => {
                let mut xs = core::mem::take(xs)
                    .into_iter()
                    .map(Self::optimise)
                    .collect::<Vec<_>>();
                if xs.len() == 1 {
                    xs.remove(0)
                } else if let Some(xs) = xs
                    .iter()
                    .map(|r| match r {
                        Regex::Char(c) => Some(*c),
                        _ => None,
                    })
                    .collect::<Option<_>>()
                {
                    Self::String(xs)
                } else if let [
                    Self::WordBoundary,
                    Self::StringSet(words),
                    Self::WordBoundary,
                ] = &mut xs[..]
                    && words.len() > 4
                /* profitability */
                {
                    words.sort();
                    words.dedup();
                    Self::WordSet(core::mem::take(words))
                } else {
                    Self::Group(xs)
                }
            }
            Self::Set(ref mut xs) => {
                let mut xs = core::mem::take(xs)
                    .into_iter()
                    .map(Self::optimise)
                    .collect::<Vec<_>>();
                if xs.len() == 1 {
                    xs.remove(0)
                } else if let Some(xs) = xs
                    .iter()
                    .map(|r| match r {
                        Regex::Char(c) => Some(*c),
                        _ => None,
                    })
                    .collect::<Option<_>>()
                {
                    Self::CharSet(xs)
                } else if let Some(xs) = xs
                    .iter()
                    .map(|r| match r {
                        Regex::Char(c) => Some(*c..=*c),
                        Regex::Range(r) => Some(r.clone()),
                        _ => None,
                    })
                    .collect::<Option<_>>()
                {
                    Self::CharRangeSet(xs)
                } else if let Some(xs) = xs
                    .iter()
                    .map(|r| match r {
                        Regex::String(s) => Some(s.clone()),
                        Regex::Char(c) => Some(c.to_string()),
                        _ => None,
                    })
                    .collect::<Option<_>>()
                {
                    Self::StringSet(xs)
                } else {
                    Self::Set(xs)
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
            | Self::String(_)
            | Self::StringSet(_)
            | Self::WordSet(_)
            | Self::CharSet(_)
            | Self::CharRangeSet(_) => self,
        }
    }

    fn prefix_inner(&self, prefix: &mut String) -> Option<()> {
        match self {
            Self::Char(c) => Some(prefix.push(*c)),
            Self::String(s) => Some(prefix.push_str(s)),
            Self::Set(xs) if xs.len() == 1 => xs[0].prefix_inner(prefix),
            Self::Group(xs) => xs.iter().map(|x| x.prefix_inner(prefix)).collect(),
            Self::AtLeastOnce(x) => x.prefix_inner(prefix),
            Self::Delim(x, _) => x.prefix_inner(prefix),
            _ => None,
        }
    }

    fn preserves_state_on_failure(&self) -> bool {
        matches!(
            self,
            Self::CharSet(_)
                | Self::CharRangeSet(_)
                | Self::Whitespace
                | Self::WordBoundary
                | Self::LineStart
                | Self::LineEnd
                | Self::Range(_)
                | Self::Char(_)
                | Self::String(_)
                | Self::StringSet(_)
        )
    }

    pub fn compile(self) -> CompiledPattern {
        let this = self.optimise();
        let mut prefix = String::new();
        this.prefix_inner(&mut prefix);
        CompiledPattern {
            regex: this.compile_cont(Opt::Return),
            prefix,
        }
    }

    fn compile_cont(self, opt: Opt) -> FastFn {
        fn cont(opt: Opt, f: impl Fn(&mut State) -> Option<()> + 'static) -> FastFn {
            match opt {
                Opt::Return => FastFn::new(f),
                Opt::Continue(next) => FastFn::new(move |state| {
                    f(state)?;
                    next.call(state)
                }),
                Opt::RepeatContinue(next) => FastFn::new(move |state| {
                    loop {
                        if state.attempt(&f).is_none() {
                            break next.call(state);
                        }
                    }
                }),
            }
        }
        match self {
            Self::Whitespace => cont(opt, move |state| {
                // SAFETY: Only matches ASCII bytes
                unsafe {
                    state.skip_byte_if(|c| c.is_ascii_whitespace())?;
                }
                while unsafe { state.skip_byte_if(|c| c.is_ascii_whitespace()).is_some() } {}
                Some(())
            }),
            Self::WordBoundary => cont(opt, move |state| state.expect_word_boundary()),
            Self::LineStart => cont(opt, move |state| {
                state.prev().map_or(true, |c| c == '\n').then_some(())
            }),
            Self::LineEnd => cont(opt, move |state| {
                state.peek().map_or(true, |c| c == '\n').then_some(())
            }),
            Self::LastDelim => cont(opt, move |state| {
                for d in state.delim.chars() {
                    state.skip_if(|c| c == d)?;
                }
                Some(())
            }),
            Self::Range(r) => {
                if r.start().is_ascii() && r.end().is_ascii() {
                    let r = *r.start() as u8..*r.end() as u8 + 1;
                    // SAFETY: `r` only matches ASCII bytes
                    cont(opt, move |state| unsafe {
                        state.skip_byte_if(|x| r.contains(&x))
                    })
                } else {
                    cont(opt, move |state| state.skip_if(|x| r.contains(&x)))
                }
            }
            Self::Char(c) => {
                if c.is_ascii() {
                    let c = c as u8;
                    // SAFETY: `c` is ASCII
                    cont(opt, move |state| unsafe { state.skip_byte_if(|x| x == c) })
                } else {
                    cont(opt, move |state| state.skip_if(|x| x == c))
                }
            }
            Self::String(s) => cont(opt, move |state| state.expect_str(&s)),
            Self::StringSet(strs) => cont(opt, move |state| {
                for s in &strs {
                    if state.expect_str(&s).is_some() {
                        return Some(());
                    }
                }
                None
            }),
            Self::WordSet(words) => cont(opt, move |state| {
                state.peek_byte()?;
                state.expect_word_boundary()?;
                let old_pos = state.pos;
                state.pos += 1;
                while state.peek_byte().is_some() && state.expect_word_boundary().is_none() {
                    state.pos += 1;
                }
                // SAFETY: We've done what's require to ensure that positions remain in-bounds and word-aligned
                let word = unsafe { state.text.get_unchecked(old_pos..state.pos) };
                if words /*.iter().any(|w| w == word){*/
                    .binary_search_by_key(&word, |w| w.as_str())
                    .is_ok()
                {
                    Some(())
                } else {
                    None
                }
            }),
            Self::CharSet(s) => {
                if s.is_ascii() {
                    cont(opt, move |state| {
                        // SAFETY: All of `xs` are ASCII ranges
                        unsafe { state.skip_byte_if(|c| s.as_bytes().contains(&c)) }
                    })
                } else {
                    cont(opt, move |state| state.skip_if(|c| s.contains(c)))
                }
            }
            Self::CharRangeSet(xs) => {
                if xs
                    .iter()
                    .all(|r| r.start().is_ascii() && r.end().is_ascii())
                {
                    let make_table = |xs: &[RangeInclusive<char>]| {
                        core::array::from_fn::<_, 256, _>(|c| {
                            xs.iter().any(|r| r.contains(&(c as u8 as char)))
                        })
                    };

                    let table = make_table(&xs);

                    if table == make_table(&['a'..='z', 'A'..='Z', '_'..='_']) {
                        cont(opt, move |state| unsafe {
                            state.skip_byte_if(|c| {
                                (b'a'..=b'z').contains(&c)
                                    || (b'A'..=b'Z').contains(&c)
                                    || c == b'_'
                            })
                        })
                    } else if table == make_table(&['a'..='z', '_'..='_']) {
                        cont(opt, move |state| unsafe {
                            state.skip_byte_if(|c| (b'a'..=b'z').contains(&c) || c == b'_')
                        })
                    } else if table == make_table(&['0'..='9']) {
                        cont(opt, move |state| unsafe {
                            state.skip_byte_if(|c| (b'0'..=b'9').contains(&c))
                        })
                    } else if xs.len() > 3 {
                        // LUT optimisation
                        cont(opt, move |state| {
                            // SAFETY: All of `xs` are ASCII ranges
                            unsafe { state.skip_byte_if(|c| table[c as usize]) }
                        })
                    } else {
                        let xs = xs
                            .into_iter()
                            .map(|r| *r.start() as u8..=*r.end() as u8)
                            .collect::<Vec<_>>();
                        cont(opt, move |state| {
                            // SAFETY: All of `xs` are ASCII ranges
                            unsafe { state.skip_byte_if(|c| xs.iter().any(|r| r.contains(&c))) }
                        })
                    }
                } else {
                    cont(opt, move |state| {
                        state.skip_if(|c| xs.iter().any(|r| r.contains(&c)))
                    })
                }
            }
            Self::Set(xs) => {
                if xs.iter().all(|x| x.preserves_state_on_failure()) {
                    let xs = xs
                        .into_iter()
                        .map(|x| x.compile_cont(Opt::Return))
                        .collect::<Vec<_>>();
                    cont(opt, move |state| xs.iter().find_map(|x| x.call(state)))
                } else {
                    let xs = xs
                        .into_iter()
                        .map(|x| x.compile_cont(Opt::Return))
                        .collect::<Vec<_>>();
                    cont(opt, move |state| {
                        xs.iter().find_map(|x| state.attempt(|s| x.call(s)))
                    })
                }
            }
            Self::NegSet(xs) => {
                let xs = xs
                    .into_iter()
                    .map(|x| x.compile_cont(Opt::Return))
                    .collect::<Vec<_>>();
                cont(opt, move |state| {
                    if xs.iter().all(|x| state.attempt(|s| x.call(s)).is_none()) {
                        state.skip_if(|_| true)?;
                        Some(())
                    } else {
                        None
                    }
                })
            }
            Self::Group(xs) => {
                let mut xs = xs.into_iter().rev();
                match xs.next() {
                    Some(last) => {
                        if opt.is_tail_only() {
                            xs.fold(last.compile_cont(opt), |tail, x| {
                                x.compile_cont(Opt::Continue(tail))
                            })
                        } else {
                            let group = xs.fold(last.compile_cont(Opt::Return), |tail, x| {
                                x.compile_cont(Opt::Continue(tail))
                            });
                            cont(opt, move |state| group.call(state))
                        }
                    }
                    None => cont(opt, move |_| Some(())),
                }
            }
            Self::Maybe(x) => {
                let x = x.compile_cont(Opt::Return);
                cont(opt, move |state| {
                    let _ = state.attempt(|s| x.call(s));
                    Some(())
                })
            }
            Self::Repeat(x) => match opt {
                Opt::Continue(next) => x.compile_cont(Opt::RepeatContinue(next)),
                _ => {
                    let x = x.compile_cont(Opt::Return);
                    cont(opt, move |state| {
                        loop {
                            let pos = state.pos;
                            if state.attempt(|s| x.call(s)).is_none() {
                                break Some(());
                            }
                            debug_assert!(pos != state.pos, "repeating but no progress");
                        }
                    })
                }
            },
            Self::AtLeastOnce(x) => {
                let op = x
                    .clone()
                    .compile_cont(Opt::Continue(Self::Repeat(x).compile_cont(Opt::Return)));
                cont(opt, move |state| op.call(state))
            }
            Self::Delim(d, r) => {
                let d = d.compile_cont(Opt::Return);
                let r = r.compile_cont(Opt::Return);
                cont(opt, move |state| {
                    let old_pos = state.pos;
                    d.call(state)?;
                    let old_delim = state.delim;
                    state.delim = &state.text[old_pos..state.pos];
                    let res = r.call(state);
                    state.delim = old_delim;
                    res
                })
            }
            Self::Rewind(r) => {
                let r = r.compile_cont(Opt::Return);
                cont(opt, move |state| {
                    let old_pos = state.pos;
                    let res = r.call(state);
                    if res.is_some() {
                        state.pos = old_pos;
                    }
                    res
                })
            }
        }
    }

    fn compile2_inner(
        self,
        out: &mut CompiledRegex2,
        ok: (Action, u16),
        fail: (Action, u16),
    ) -> usize {
        match self {
            Self::Whitespace => out.populate(|c| c.is_ascii_whitespace(), ok, fail),
            Self::Char(x) => out.populate(|c| c as char == x, ok, fail),
            Self::CharSet(xs) => out.populate(|c| xs.contains(c as char), ok, fail),
            Self::CharRangeSet(xs) => {
                out.populate(|c| xs.iter().any(|x| x.contains(&(c as char))), ok, fail)
            }
            Self::Range(r) => out.populate(|c| r.contains(&(c as char)), ok, fail),
            Self::Group(xs) => {
                let mut ok = ok;
                for x in xs.into_iter().rev() {
                    ok = (Action::Continue, x.compile2_inner(out, ok, fail) as u16);
                }
                ok.1 as usize
            }
            Self::Repeat(x) => {
                let old_tables = out.tables.len();
                let addr = x.compile2_inner(out, (Action::Continue, CompiledRegex2::FIXUP), ok);
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
        out.entry = self.compile2_inner(&mut out, (Action::Ok, 0), (Action::Fail, 0));
        out
    }
}

enum Opt {
    // Return after the the current regex is done
    Return,
    // Continue to the given regex
    Continue(FastFn),
    // Repeat the current regex, then continue to the given one
    RepeatContinue(FastFn),
}

impl Opt {
    // Some optimisations only need applying to the tail parser (the final parser in the state machine)
    fn is_tail_only(&self) -> bool {
        matches!(self, Self::Return | Self::Continue(_))
    }
}

pub struct CompiledRegex2 {
    tables: Vec<[(Action, u16); 256]>,
    entry: usize,
}

#[derive(Copy, Clone)]
#[repr(u8)]
enum Action {
    Continue,
    Ok,
    Fail,
}

impl CompiledRegex2 {
    const FIXUP: u16 = 0xFFFF;

    fn populate(
        &mut self,
        f: impl Fn(u8) -> bool,
        ok: (Action, u16),
        fail: (Action, u16),
    ) -> usize {
        let idx = self.tables.len();
        self.tables
            .push(core::array::from_fn(|x| if f(x as u8) { ok } else { fail }));
        idx
    }

    pub fn matches(&self, s: &str, mut at: usize) -> Option<usize> {
        let mut next = self.entry;
        loop {
            let b = s.as_bytes().get(at).copied().unwrap_or(0);
            let (action, goto) = self.tables[next as usize][b as usize];
            next = goto as usize;
            match action {
                Action::Continue => at += 1,
                Action::Ok => return Some(at),
                Action::Fail => return None,
            }
        }
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
