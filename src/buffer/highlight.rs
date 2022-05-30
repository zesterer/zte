use std::{
    ops::Range,
    path::Path,
};

#[derive(Default)]
pub struct Highlights {
    regions: Vec<(Range<usize>, Region)>,
}

impl Highlights {
    pub fn get_at(&self, pos: usize) -> Region {
        let mut spos = 0;
        let mut slen = self.regions.len();

        loop {
            let (range, region) = match self.regions.get(spos + slen / 2) {
                Some(x) => x,
                None => return Region::Normal,
            };

            if range.contains(&pos) {
                return *region;
            } else if slen <= 1 {
                return Region::Normal;
            } else if range.start >= pos {
                slen /= 2;
            } else {
                spos += slen / 2;
                slen = slen - slen / 2;
            }
        }
    }
}

impl Highlights {
    pub fn from_file(path: Option<&Path>, src: &str) -> Self {
        let regions = match path.and_then(|p| p.extension()?.to_str()) {
            Some("rs") => RustToken::lexer(src)
                .spanned()
                .map(|(tok, span)| (span, match tok {
                    RustToken::Other => Region::Normal,
                    RustToken::Token(r) => r,
                }))
                .collect(),
            _ => return Self::default(),
        };
        Self { regions }
    }
}

/*
const PRIMITIVES: [&str; 17] = [
	"usize", "isize",
	"u8", "u16", "u32", "u64", "u128",
	"i8", "i16", "i32", "i64", "i128",
	"f32", "f64",
	"str", "bool", "char",
];
*/

#[derive(Copy, Clone, Debug)]
pub enum Region {
    Normal,
    Keyword,
    LineComment,
    MultiComment,
    Label,
    Primitive,
    Symbol,
    Bracket,
    Numeric,
    String,
    Macro,
}

use logos::Logos;

#[derive(Logos)]
enum RustToken {
    #[regex(r"[a-zA-Z_][0-9a-zA-Z_]+!", |_| Region::Macro, priority = 0)]
    #[regex(r"[a-zA-Z_][0-9a-zA-Z_]+", |_| Region::Normal, priority = 0)]
    #[regex(r"'[a-zA-Z_][0-9a-zA-Z_]+", |_| Region::Label, priority = 0)]
    #[regex(r"//[^\n\r]*", |_| Region::LineComment, priority = 0)]
    #[regex(r"/[*][^*/]*[*]/", |_| Region::MultiComment, priority = 0)]
    #[regex(r"[0-9][.[0-9]]?", |_| Region::Numeric, priority = 0)]
    #[regex(r#"["][^"]*""#, |_| Region::String, priority = 0)]
    #[token(r"+", |_| Region::Symbol)]
    #[token(r"-", |_| Region::Symbol)]
    #[token(r"*", |_| Region::Symbol)]
    #[token(r"/", |_| Region::Symbol)]
    #[token(r"%", |_| Region::Symbol)]
    #[token(r"[", |_| Region::Symbol)]
    #[token(r"]", |_| Region::Symbol)]
    #[token(r"{", |_| Region::Symbol)]
    #[token(r"}", |_| Region::Symbol)]
    #[token(r"(", |_| Region::Symbol)]
    #[token(r")", |_| Region::Symbol)]
    #[token(r"<", |_| Region::Symbol)]
    #[token(r">", |_| Region::Symbol)]
    #[token(r"=", |_| Region::Symbol)]
    #[token(r"&", |_| Region::Symbol)]
    #[token(r"@", |_| Region::Symbol)]
    #[token(r".", |_| Region::Symbol)]
    #[token(r":", |_| Region::Symbol)]
    #[token("struct", |_| Region::Keyword)]
    #[token("enum", |_| Region::Keyword)]
    #[token("use", |_| Region::Keyword)]
    #[token("match", |_| Region::Keyword)]
    #[token("if", |_| Region::Keyword)]
    #[token("else", |_| Region::Keyword)]
    #[token("loop", |_| Region::Keyword)]
    #[token("while", |_| Region::Keyword)]
    #[token("let", |_| Region::Keyword)]
    #[token("fn", |_| Region::Keyword)]
    #[token("pub", |_| Region::Keyword)]
    #[token("continue", |_| Region::Keyword)]
    #[token("break", |_| Region::Keyword)]
    #[token("return", |_| Region::Keyword)]
    #[token("as", |_| Region::Keyword)]
    #[token("const", |_| Region::Keyword)]
    #[token("crate", |_| Region::Keyword)]
    #[token("extern", |_| Region::Keyword)]
    #[token("true", |_| Region::Keyword)]
    #[token("false", |_| Region::Keyword)]
    #[token("impl", |_| Region::Keyword)]
    #[token("for", |_| Region::Keyword)]
    #[token("in", |_| Region::Keyword)]
    #[token("mod", |_| Region::Keyword)]
    #[token("move", |_| Region::Keyword)]
    #[token("mut", |_| Region::Keyword)]
    #[token("ref", |_| Region::Keyword)]
    #[token("self", |_| Region::Keyword)]
    #[token("Self", |_| Region::Keyword)]
    #[token("static", |_| Region::Keyword)]
    #[token("trait", |_| Region::Keyword)]
    #[token("type", |_| Region::Keyword)]
    #[token("unsafe", |_| Region::Keyword)]
    #[token("where", |_| Region::Keyword)]
    Token(Region),

    #[error]
    #[regex(r"[ \t\n\f]+", logos::skip)]
    Other,
}
