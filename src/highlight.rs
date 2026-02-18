use super::{lang::LangPack, state::Text, *};
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
    // matchers: Vec<Regex>,
    matchers: Vec<regex::CompiledPattern>,
    entries: Vec<(TokenKind, Option<Highlighter>)>,
}

impl Highlighter {
    pub fn with(mut self, token: TokenKind, p: impl AsRef<str>) -> Self {
        self.entries.push((token, None));
        self.matchers
            .push(regex::CompiledPattern::create(p.as_ref()));
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
            .push(regex::CompiledPattern::create(p.as_ref()));
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
                && n > i
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
                n
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

#[test]
fn rust() {
    let s = include_str!("state.rs");
    let lang = LangPack::from_file_name(std::path::Path::new("state.rs"));

    lang.highlighter.highlight_str(&s[..], 0..s.len());
}
