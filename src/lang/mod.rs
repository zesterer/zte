use super::*;
use crate::highlight::{Highlighter, TokenKind};
use std::path::Path;

#[derive(Default)]
pub struct LangPack {
    pub highlighter: Highlighter,
    pub comment_syntax: Option<Vec<char>>,
}

impl LangPack {
    pub fn from_file_name(file_name: &Path) -> Self {
        match file_name.extension().and_then(|e| e.to_str()).unwrap_or("") {
            "rs" => Self {
                highlighter: Highlighter::default().rust(),
                comment_syntax: Some(vec!['/', '/', ' ']),
            },
            "md" => Self {
                highlighter: Highlighter::default().markdown(),
                comment_syntax: None,
            },
            "toml" => Self {
                highlighter: Highlighter::default().toml(),
                comment_syntax: Some(vec!['#', ' ']),
            },
            "c" | "h" | "cpp" | "hpp" | "cxx" | "js" | "ts" | "go" => Self {
                highlighter: Highlighter::default().generic_clike(),
                comment_syntax: Some(vec!['/', '/', ' ']),
            },
            "glsl" | "vert" | "frag" => Self {
                highlighter: Highlighter::default().glsl(),
                comment_syntax: Some(vec!['/', '/', ' ']),
            },
            "py" => Self {
                highlighter: Highlighter::default().python(),
                comment_syntax: Some(vec!['#', ' ']),
            },
            _ => Self {
                highlighter: Highlighter::default(),
                comment_syntax: None,
            },
        }
    }
}

impl Highlighter {
    pub fn markdown(self) -> Self {
        self
            // Links
            .with(TokenKind::String, r"\[[^\]]*\](\([^\)]*\))?")
            // Header
            .with(TokenKind::Doc, r"^#+[[:space:]][^$]*$")
            // List item
            .with(TokenKind::Operator, r"^[[:space:]]?[\-([0-9]+[\)\.])]")
            // Bold
            .with(TokenKind::Property, r"\*\*[^(\*\*)]*\*\*")
            // Italics
            .with(TokenKind::Attribute, r"\*[^\*]*\*")
            // Code block
            .with(TokenKind::Operator, r"^```[^(^```)]*^```")
            // Inline code
            .with(TokenKind::Constant, r"`[^`$]*[`$]")
            // HTML
            .with(TokenKind::Special, r"<[^<>]*>")
    }

    pub fn rust(self) -> Self {
        self
            // Both kinds of comments match multiple lines
            .with(TokenKind::Doc, r"\/\/[\/!][^\n]*$(\n[[:space:]]\/\/[\/!][^\n]*$)*")
            .with(TokenKind::Comment, r"\/\/[^$]*$(\n[[:space:]]\/\/[^$]*$)*")
            // Multi-line comment
            .with(TokenKind::Comment, r"\/\*[^(\*\/)]*\*\/")
            .with(
                TokenKind::Keyword,
                r"\b[(pub)(enum)(let)(self)(Self)(fn)(impl)(struct)(use)(if)(while)(for)(in)(loop)(mod)(match)(else)(break)(continue)(trait)(const)(static)(type)(mut)(as)(crate)(extern)(move)(ref)(return)(super)(unsafe)(use)(where)(async)(dyn)(try)(gen)(macro_rules)(union)(raw)]\b",
            )
            .with(TokenKind::Constant, r"\b[(true)(false)]\b")
            // Flow-control operators count as keywords
            .with(TokenKind::Keyword, r"\.await\b")
            // Macro invocations: println!
            .with(TokenKind::Macro, r"\b[A-Za-z_][A-Za-z0-9_]*!")
            // Meta-variables
            .with(TokenKind::Macro, r"\$[A-Za-z_][A-Za-z0-9_]*\b")
            .with(TokenKind::Constant, r"\b[A-Z][A-Z0-9_]+\b")
            .with(TokenKind::Type, r"\b[A-Z][A-Za-z0-9_]*\b")
            // Primitives
            .with(
                TokenKind::Type,
                r"\b[(u8)(u16)(u32)(u64)(u128)(i8)(i16)(i32)(i64)(i128)(usize)(isize)(bool)(str)(char)(f16)(f32)(f64)(f128)]\b",
            )
            // "foo" or b"foo" or r#"foo"#
            .with(TokenKind::String, r#"b?r?(#*)@("[(\\")[^("~)]]*("~))"#)
            // Characters
            .with(
                TokenKind::String,
                r#"b?'[(\\[nrt\\0(x[0-7A-Za-z][0-7A-Za-z])])[^']]'"#,
            )
            .with(
                TokenKind::Operator,
                r"[(&(mut)?)(\?)(\+=?)(\-=?)(\*=?)(\/=?)(%=?)(!=?)(==?)(&&?=?)(\|\|?=?)(<<?=?)(>>?=?)(\.\.[\.=]?)\\\~\^:;,\@(=>?)]",
            )
            // Fields and methods: a.foo
            .with(TokenKind::Property, r"\.[a-z_][A-Za-z0-9_]*")
            // Paths: std::foo::bar
            .with(TokenKind::Property, r"[A-Za-z_][A-Za-z0-9_]*::")
            // Lifetimes
            .with(TokenKind::Special, r"'[a-z_][A-Za-z0-9_]*\b")
            .with(TokenKind::Ident, r"\b[a-z_][A-Za-z0-9_]*\b")
            .with(TokenKind::Number, r"[0-9][A-Za-z0-9_\.]*")
            .with(TokenKind::Delimiter, r"[\{\}\(\)\[\]]")
            .with(TokenKind::Macro, r"[\{\}\(\)\[\]]")
            .with(TokenKind::Attribute, r"#!?\[[^\]]*\]")
    }

    fn clike_comments(self) -> Self {
        self
            // Both kinds of comments match multiple lines
            .with(
                TokenKind::Doc,
                r"\/\/[\/!][^\n]*$(\n[[:space:]]\/\/[\/!][^\n]*$)*",
            )
            // Regular comment
            .with(TokenKind::Comment, r"\/\/[^$]*$(\n[[:space:]]\/\/[^$]*$)*")
            // Multi-line comment
            .with(TokenKind::Comment, r"\/\*[^(\*\/)]*\*\/")
    }

    fn clike_preprocessor(self) -> Self {
        self.with(TokenKind::Macro, r"^#[^$]*$")
    }

    pub fn clike(self) -> Self {
        self
            .with(TokenKind::Constant, r"\b[(true)(false)]\b")
            .with(TokenKind::Constant, r"\b[A-Z][A-Z0-9_]+\b")
            .with(TokenKind::Type, r"\b[A-Z][A-Za-z0-9_]*\b")
            // "foo" or b"foo" or r#"foo"#
            .with(TokenKind::String, r#"b?r?(#*)@("[(\\")[^("~)]]*("~))"#)
            // Character strings
            .with(TokenKind::String, r#"b?'[(\\[nrt\\0(x[0-7A-Za-z][0-7A-Za-z])])[^']]*'"#)
            .with(
                TokenKind::Operator,
                r"[(&)(\?)(\+\+)(\-\-)(\+=?)(\-=?)(\*=?)(\/=?)(%=?)(!=?)(==?)(&&?=?)(\|\|?=?)(<<?=?)(>>?=?)(\.\.[\.=]?)\\\~\^:;,\@(=>?)]",
            )
            // Fields and methods: a.foo
            .with(TokenKind::Property, r"\.[a-z_][A-Za-z0-9_]*")
            // Paths: std::foo::bar
            .with(TokenKind::Property, r"[A-Za-z_][A-Za-z0-9_]*::")
            .with(TokenKind::Ident, r"\b[a-z_][A-Za-z0-9_]*\b")
            .with(TokenKind::Number, r"[0-9][A-Za-z0-9_\.]*")
            .with(TokenKind::Delimiter, r"[\{\}\(\)\[\]]")
    }

    pub fn generic_clike(self) -> Self {
        self
            // Keywords
            .with(TokenKind::Keyword, r"\b[(var)(enum)(let)(this)(fn)(struct)(class)(import)(if)(while)(for)(in)(loop)(else)(break)(continue)(const)(static)(type)(extern)(return)(async)(throw)(catch)(union)(auto)(namespace)(public)(private)(function)(func)]\b")
            // Primitives
            .with(TokenKind::Type, r"\b[(([(unsigned)(signed)][[:space:]])*u?int[0-9]*(_t)?)(float)(double)(bool)(char)(size_t)(void)]\b")
            .clike_comments()
            .clike_preprocessor()
            .clike()
    }

    pub fn glsl(self) -> Self {
        self
            // Keywords
            .with(TokenKind::Keyword, r"\b[(struct)(if)(while)(for)(else)(break)(continue)(const)(return)(layout)(uniform)(set)(binding)(location)(in)]\b")
            // Primitives
            .with(TokenKind::Type, r"\b[(u?int)(float)(double)(bool)(void)([ui]?vec[1-4]*)([ui]?mat[1-4]*)(texture[(2D)(3D)]?(Cube)?)([ui]?sampler[(2D)(3D)]?(Shadow)?)]\b")
            // Builtins
            .with(TokenKind::Macro, r"\b[(dot)(cross)(textureSize)(normalize)(texelFetch)(textureProj)(max)(min)(clamp)(reflect)(mix)(distance)(length)(abs)(pow)(sign)(sin)(cos)(tan)(fract)(mod)(round)(step)]\b")
            .clike_comments()
            .clike_preprocessor()
            .clike()
    }

    pub fn python(self) -> Self {
        self
            // Keywords
            .with(TokenKind::Keyword, r"\b[(and)(as)(assert)(break)(class)(continue)(def)(del)(elif)(else)(except)(finally)(for)(from)(global)(if)(import)(in)(is)(lambda)(nonlocal)(not)(or)(pass)(raise)(return)(try)(while)(with)(yield)]\b")
            // Primitives
            .with(TokenKind::Type, r"\b[]\b")
            // Builtins
            .with(TokenKind::Macro, r"\b[(True)(False)(None)]\b")
            // Doc comments
            .with(TokenKind::Doc, r"^##[^$]*$")
            // Comments
            .with(TokenKind::Comment, r"^#[^$]*$")
            .clike()
    }

    pub fn toml(self) -> Self {
        self
            // Header
            .with(TokenKind::Doc, r#"^\[[^\n\]]*\]$"#)
            // Delimiters
            .with(TokenKind::Delimiter, r"[\{\}\(\)\[\]]")
            // Operators
            .with(TokenKind::Operator, r"[=,]")
            // Numbers
            .with(TokenKind::Number, r"[0-9][A-Za-z0-9_\.]*")
            // Double-quoted strings
            .with(
                TokenKind::String,
                r#"b?"[(\\[nrt\\0(x[0-7A-Za-z][0-7A-Za-z])])[^"]]*""#,
            )
            // Single-quoted strings
            .with(
                TokenKind::String,
                r#"b?'[(\\[nrt\\0(x[0-7A-Za-z][0-7A-Za-z])])[^']]*'"#,
            )
            // Booleans
            .with(TokenKind::Constant, r"\b[(true)(false)]\b")
            // Identifier
            .with(TokenKind::Ident, r"\b[a-z_][A-Za-z0-9_\-]*\b")
            // Comments
            .with(TokenKind::Comment, r"#[^$]*$")
    }
}
