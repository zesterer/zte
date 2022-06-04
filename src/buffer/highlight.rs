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
        let indices = src.char_indices().map(|(i, _)| i).collect::<Vec<_>>();
        let byte_to_char_idx = |idx| indices.binary_search(&idx).unwrap_or_else(|i| i);
        let regions = match path.and_then(|p| p.extension()?.to_str()) {
            Some("rs") | Some("ron") => RustToken::lexer(src)
                .spanned()
                .map(|(tok, span)| (span, match tok {
                    RustToken::Other => Region::Normal,
                    RustToken::Token(r) => r,
                }))
                .collect(),
            Some("tao") => TaoToken::lexer(src)
                .spanned()
                .map(|(tok, span)| (span, match tok {
                    TaoToken::Other => Region::Normal,
                    TaoToken::Token(r) => r,
                }))
                .collect(),
            Some("toml") => TomlToken::lexer(src)
                .spanned()
                .map(|(tok, span)| (tok, byte_to_char_idx(span.start)..byte_to_char_idx(span.end)))
                .map(|(tok, span)| (span, match tok {
                    TomlToken::Other => Region::Normal,
                    TomlToken::Token(r) => r,
                }))
                .collect(),
            Some("md") => MdToken::lexer(src)
                .spanned()
                .map(|(tok, span)| (tok, byte_to_char_idx(span.start)..byte_to_char_idx(span.end)))
                .map(|(tok, span)| (span, match tok {
                    MdToken::Other => Region::Normal,
                    MdToken::Token(r) => r,
                }))
                .collect(),
            Some("log") => LogToken::lexer(src)
                .spanned()
                .map(|(tok, span)| (tok, byte_to_char_idx(span.start)..byte_to_char_idx(span.end)))
                .map(|(tok, span)| (span, match tok {
                    LogToken::Other => Region::Normal,
                    LogToken::Token(r) => r,
                }))
                .collect(),
            Some("glsl") | Some("vert") | Some("frag") => GlslToken::lexer(src)
                .spanned()
                .map(|(tok, span)| (tok, byte_to_char_idx(span.start)..byte_to_char_idx(span.end)))
                .map(|(tok, span)| (span, match tok {
                    GlslToken::Other => Region::Normal,
                    GlslToken::Token(r) => r,
                }))
                .collect(),
            _ => return Self::default(),
        };
        Self { regions }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Region {
    Normal,
    Property,
    Keyword,
    LineComment,
    MultiComment,
    Label,
    Symbol,
    Bracket,
    Block,
    Numeric,
    String,
    Macro,
    Type,
    Constant,
    Path,
    Error,
    Warning,
    Info,
}

use logos::Logos;

#[derive(Logos)]
enum RustToken {
    #[regex(r"[a-zA-Z_][0-9a-zA-Z_]*!", |_| Region::Macro, priority = 0)]
    #[regex(r"[a-z_][0-9a-zA-Z_]*", |_| Region::Normal, priority = 1)]
    #[regex(r"\.[a-z_][0-9a-zA-Z_]*", |_| Region::Property, priority = 1)]
    //#[regex(r"[a-z_][0-9a-zA-Z_]*::", |_| Region::Path, priority = 0)]
    #[regex(r"'[a-zA-Z_][0-9a-zA-Z_]*", |_| Region::Label, priority = 0)]
    #[regex(r"//[^\n\r]*", |_| Region::LineComment, priority = 0)]
    #[regex(r"/\*[^*]*\*+(?:[^/*][^*]*\*+)*/", |_| Region::MultiComment, priority = 0)]
    #[regex(r"[[0b]|[0o]]?[0-9]+[.[0-9]]?", |_| Region::Numeric, priority = 0)]
    #[regex(r"0x[0-9a-fA-F]+[.[0-9a-fA-F]]?", |_| Region::Numeric, priority = 0)]
    #[regex(r#"["][^"]*""#, |_| Region::String, priority = 0)]
    #[regex(r"#\[[^\]]*\]", |_| Region::Macro, priority = 0)]
    #[regex(r"#!\[[^\]]*\]", |_| Region::Macro, priority = 0)]
    #[regex(r"[A-Z][0-9A-Z_]+", |_| Region::Constant, priority = 1)]
    #[token(r"+", |_| Region::Symbol)]
    #[token(r"-", |_| Region::Symbol)]
    #[token(r"*", |_| Region::Symbol)]
    #[token(r"/", |_| Region::Symbol)]
    #[token(r"%", |_| Region::Symbol)]
    #[token(r"[", |_| Region::Bracket)]
    #[token(r"]", |_| Region::Bracket)]
    #[token(r"{", |_| Region::Block)]
    #[token(r"}", |_| Region::Block)]
    #[token(r"(", |_| Region::Bracket)]
    #[token(r")", |_| Region::Bracket)]
    #[token(r"<", |_| Region::Symbol)]
    #[token(r">", |_| Region::Symbol)]
    #[token(r"=", |_| Region::Symbol)]
    #[token(r"&", |_| Region::Symbol)]
    #[token(r"@", |_| Region::Symbol)]
    #[token(r".", |_| Region::Symbol)]
    #[token(r":", |_| Region::Symbol)]
    #[token(r"?", |_| Region::Symbol)]
    #[token(r",", |_| Region::Symbol)]
    #[token("struct", |_| Region::Keyword)]
    #[token("enum", |_| Region::Keyword)]
    #[token("use", |_| Region::Keyword)]
    #[token("match", |_| Region::Keyword)]
    #[token("if", |_| Region::Keyword)]
    #[token("else", |_| Region::Keyword)]
    #[token("loop", |_| Region::Keyword)]
    #[token("while", |_| Region::Keyword)]
    #[token("for", |_| Region::Keyword)]
    #[token("let", |_| Region::Keyword)]
    #[token("fn", |_| Region::Keyword)]
    #[token("pub", |_| Region::Keyword)]
    #[token("super", |_| Region::Keyword)]
    #[token("continue", |_| Region::Keyword)]
    #[token("break", |_| Region::Keyword)]
    #[token("return", |_| Region::Keyword)]
    #[token("as", |_| Region::Keyword)]
    #[token("const", |_| Region::Keyword)]
    #[token("crate", |_| Region::Keyword)]
    #[token("extern", |_| Region::Keyword)]
    #[token("true", |_| Region::Numeric)]
    #[token("false", |_| Region::Numeric)]
    #[token("dyn", |_| Region::Keyword)]
    #[token("async", |_| Region::Keyword)]
    #[token("await", |_| Region::Keyword)]
    #[token("impl", |_| Region::Keyword)]
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
    #[regex(r"[A-Z][0-9a-zA-Z_]*", |_| Region::Type, priority = 0)]
    #[token("usize", |_| Region::Type)]
    #[token("isize", |_| Region::Type)]
    #[regex("u[0-9]+", |_| Region::Type)]
    #[regex("i[0-9]+", |_| Region::Type)]
    #[regex("f[0-9]+", |_| Region::Type)]
    #[token("str", |_| Region::Type)]
    #[token("bool", |_| Region::Type)]
    #[token("char", |_| Region::Type)]
    Token(Region),

    #[error]
    #[regex(r"[ \t\n\f]+", logos::skip)]
    Other,
}

#[derive(Logos)]
enum TaoToken {
    #[regex(r"@[a-zA-Z_][0-9a-zA-Z_]*", |_| Region::Macro, priority = 0)]
    #[regex(r"[a-z_][0-9a-zA-Z_]*", |_| Region::Normal, priority = 1)]
    #[regex(r"\.[a-z_][0-9a-zA-Z_]*", |_| Region::Property, priority = 1)]
    #[regex(r"#[^\n\r]*", |_| Region::LineComment, priority = 0)]
    #[regex(r"#[#|!][^\n\r]*", |_| Region::String, priority = 1)]
    #[regex(r"[[0b]|[0o]]?[0-9]+[.[0-9]]?", |_| Region::Numeric, priority = 0)]
    #[regex(r"0x[0-9a-fA-F]+[.[0-9a-fA-F]]?", |_| Region::Numeric, priority = 0)]
    #[regex(r#"["][^"]*""#, |_| Region::String, priority = 0)]
    #[regex(r"\$\[[^\]]*\]", |_| Region::Macro, priority = 0)]
    #[token(r"+", |_| Region::Symbol)]
    #[token(r"-", |_| Region::Symbol)]
    #[token(r"*", |_| Region::Symbol)]
    #[token(r"/", |_| Region::Symbol)]
    #[token(r"%", |_| Region::Symbol)]
    #[token(r"[", |_| Region::Bracket)]
    #[token(r"]", |_| Region::Bracket)]
    #[token(r"{", |_| Region::Block)]
    #[token(r"}", |_| Region::Block)]
    #[token(r"(", |_| Region::Bracket)]
    #[token(r")", |_| Region::Bracket)]
    #[token(r"|", |_| Region::Bracket)]
    #[token(r"\", |_| Region::Bracket)]
    #[token(r"<", |_| Region::Symbol)]
    #[token(r">", |_| Region::Symbol)]
    #[token(r"=", |_| Region::Symbol)]
    #[token(r"&", |_| Region::Symbol)]
    #[token(r"@", |_| Region::Symbol)]
    #[token(r".", |_| Region::Symbol)]
    #[token(r":", |_| Region::Symbol)]
    #[token(r"?", |_| Region::Symbol)]
    #[token(r",", |_| Region::Symbol)]
    #[token(r"~", |_| Region::Symbol)]
    #[token("data", |_| Region::Keyword)]
    #[token("member", |_| Region::Keyword)]
    #[token("def", |_| Region::Keyword)]
    #[token("class", |_| Region::Keyword)]
    #[token("type", |_| Region::Keyword)]
    #[token("effect", |_| Region::Keyword)]
    #[token("import", |_| Region::Keyword)]
    #[token("handle", |_| Region::Keyword)]
    #[token("with", |_| Region::Keyword)]
    #[token("match", |_| Region::Keyword)]
    #[token("if", |_| Region::Keyword)]
    #[token("else", |_| Region::Keyword)]
    #[token("for", |_| Region::Keyword)]
    #[token("of", |_| Region::Keyword)]
    #[token("let", |_| Region::Keyword)]
    #[token("fn", |_| Region::Keyword)]
    #[token("return", |_| Region::Keyword)]
    #[token("in", |_| Region::Keyword)]
    #[token("mod", |_| Region::Keyword)]
    #[token("where", |_| Region::Keyword)]
    #[regex(r"[A-Z][0-9a-zA-Z_]*", |_| Region::Type, priority = 0)]
    #[token("usize", |_| Region::Type)]
    #[token("isize", |_| Region::Type)]
    #[regex("u[0-9]+", |_| Region::Type)]
    #[regex("i[0-9]+", |_| Region::Type)]
    #[regex("f[0-9]+", |_| Region::Type)]
    #[token("str", |_| Region::Type)]
    #[token("bool", |_| Region::Type)]
    #[token("char", |_| Region::Type)]
    Token(Region),

    #[error]
    #[regex(r"[ \t\n\f]+", logos::skip)]
    Other,
}

#[derive(Logos)]
enum GlslToken {
    #[regex(r"[a-zA-Z_][0-9a-zA-Z_]*!", |_| Region::Macro, priority = 0)]
    #[regex(r"[a-z_][0-9a-zA-Z_]*", |_| Region::Normal, priority = 0)]
    #[regex(r"//[^\n\r]*", |_| Region::LineComment, priority = 0)]
    #[regex(r"/\*[^*]*\*+(?:[^/*][^*]*\*+)*/", |_| Region::MultiComment, priority = 0)]
    #[regex(r"[[0b]|[0o]]?[0-9]+[.[0-9]]?u?", |_| Region::Numeric, priority = 1)]
    #[regex(r"0x[0-9a-fA-F]+[.[0-9a-fA-F]]?u?", |_| Region::Numeric, priority = 1)]
    #[regex(r#"["][^"]*""#, |_| Region::String, priority = 0)]
    #[regex(r"[A-Z][0-9A-Z_]+", |_| Region::Constant, priority = 100)]
    #[regex(r"#[a-z]+", |_| Region::Macro, priority = 0)]
    #[regex(r"gl_[0-9a-zA-Z_]*", |_| Region::Constant, priority = 1)]
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
    #[token(r"?", |_| Region::Symbol)]
    #[token(r",", |_| Region::Symbol)]
    #[token("struct", |_| Region::Keyword)]
    #[token("enum", |_| Region::Keyword)]
    #[token("layout", |_| Region::Keyword)]
    #[token("uniform", |_| Region::Keyword)]
    #[token("if", |_| Region::Keyword)]
    #[token("else", |_| Region::Keyword)]
    #[token("for", |_| Region::Keyword)]
    #[token("while", |_| Region::Keyword)]
    #[token("continue", |_| Region::Keyword)]
    #[token("break", |_| Region::Keyword)]
    #[token("return", |_| Region::Keyword)]
    #[token("in", |_| Region::Keyword)]
    #[token("out", |_| Region::Keyword)]
    #[token("inout", |_| Region::Keyword)]
    #[token("flat", |_| Region::Keyword)]
    #[token("smooth", |_| Region::Keyword)]
    #[token("discard", |_| Region::Keyword)]
    #[token("const", |_| Region::Keyword)]
    #[token("true", |_| Region::Keyword)]
    #[token("false", |_| Region::Keyword)]
    #[regex(r"[A-Z][0-9a-zA-Z_]*", |_| Region::Type, priority = 2)]
    #[regex("vec[0-9]", |_| Region::Type)]
    #[regex("ivec[0-9]", |_| Region::Type)]
    #[regex("uvec[0-9]", |_| Region::Type)]
    #[regex("bvec[0-9]", |_| Region::Type)]
    #[token("int", |_| Region::Type)]
    #[token("uint", |_| Region::Type)]
    #[token("float", |_| Region::Type)]
    #[token("double", |_| Region::Type)]
    #[token("bool", |_| Region::Type)]
    #[token("void", |_| Region::Type)]
    Token(Region),

    #[error]
    #[regex(r"[ \t\n\f]+", logos::skip)]
    Other,
}

#[derive(Logos)]
enum TomlToken {
    #[regex(r"[a-zA-Z_][0-9a-zA-Z_\-]+", |_| Region::Macro, priority = 0)] // Key
    #[regex(r"\[[0-9a-zA-Z_-]+\]", |_| Region::Label, priority = 0)]
    #[regex(r"#[^\n\r]*", |_| Region::LineComment, priority = 0)]
    #[regex(r"[0-9][.[0-9]]?", |_| Region::Numeric, priority = 0)]
    #[regex(r#"["][^"]*""#, |_| Region::String, priority = 100)]
    Token(Region),

    #[error]
    #[regex(r"[ \t\n\f]+", logos::skip)]
    Other,
}

#[derive(Logos)]
enum LogToken {
    #[regex(r"error|ERROR", |_| Region::Error)]
    #[regex(r"warn|WARN", |_| Region::Warning)]
    #[regex(r"info|INFO", |_| Region::Info)]
    #[regex(r"trace|TRACE", |_| Region::LineComment)]
    Token(Region),

    #[error]
    #[regex(r"[ \t\n\f]+", logos::skip)]
    Other,
}

#[derive(Logos)]
enum MdToken {
    //#[regex(r"[^\n\r]*", |_| Region::MultiComment, priority = 0)]
    #[regex(r"#[^\n\r]*", |_| Region::Macro, priority = 1)]
    #[regex(r"##[^\n\r]*", |_| Region::Macro, priority = 2)]
    #[regex(r"###[^\n\r]*", |_| Region::Macro, priority = 3)]
    #[regex(r#"\[[^\]]*\](\([^\)]*\))?"#, |_| Region::String, priority = 100)]
    #[regex(r#"[ \t\n\f]*-"#, |_| Region::Symbol, priority = 1)]
    #[regex(r#"```[^[```]]*```"#, |_| Region::Numeric, priority = 100)]
    #[regex(r#"`[^`]*`"#, |_| Region::Numeric, priority = 100)]
    #[regex(r#"\*[^\*]*\*"#, |_| Region::Info, priority = 100)]
    Token(Region),

    #[error]
    #[regex(r"[ \t\n\f]+", logos::skip)]
    Other,
}
