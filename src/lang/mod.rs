use crate::highlight::{Highlighter, TokenKind};
use std::path::Path;

#[derive(Default)]
pub struct LangPack {
    pub highlighter: Highlighter,
    pub comment_syntax: Option<String>,
    pub delims: Vec<(char, char)>,
    pub reflow_col: Option<usize>,
}

impl LangPack {
    pub fn from_file_name(file_name: &Path) -> Self {
        let fname = file_name.file_name().and_then(|e| e.to_str()).unwrap_or("");
        let fprefix = file_name
            .file_prefix()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let fextension = file_name.extension().and_then(|e| e.to_str()).unwrap_or("");

        if matches!(fextension, "rs" | "ron") {
            Self::clike(Highlighter::code().rust())
        } else if matches!(fextension, "md") {
            Self {
                highlighter: Highlighter::code().markdown(),
                reflow_col: None, //Some(88),
                ..Default::default()
            }
        } else if matches!(fname, "Cargo.lock") || matches!(fextension, "toml") {
            Self {
                highlighter: Highlighter::code().toml(),
                comment_syntax: Some("# ".to_string()),
                delims: vec![('(', ')'), ('{', '}'), ('[', ']')],
                ..Default::default()
            }
        } else if matches!(fextension, "yaml" | "yml") {
            Self {
                highlighter: Highlighter::code().yaml(),
                comment_syntax: Some("# ".to_string()),
                ..Default::default()
            }
        } else if matches!(fextension, "c" | "h" | "cpp" | "hpp" | "cxx") {
            Self::clike(Highlighter::code().cpp())
        } else if matches!(fextension, "js" | "ts" | "go") {
            Self::clike(Highlighter::code().generic_clike())
        } else if matches!(fextension, "glsl" | "vert" | "frag") {
            Self::clike(Highlighter::code().glsl())
        } else if matches!(fextension, "py") {
            Self::pythonic(Highlighter::code().python())
        } else if matches!(fextension, "tao") {
            Self::pythonic(Highlighter::code().tao())
        } else if matches!(fname, "makefile" | "Makefile") || matches!(fextension, "mk") {
            Self::pythonic(Highlighter::code().makefile())
        } else if matches!(fextension, "proto" | "json") {
            Self::clike(Highlighter::code().clike_comments().generic_delimited())
        } else if matches!(fextension, "sh" | "bash" | "zsh") {
            Self {
                highlighter: Highlighter::code().shell(),
                comment_syntax: Some("# ".to_string()),
                ..Default::default()
            }
        } else if matches!(fprefix, "Dockerfile") {
            Self {
                highlighter: Highlighter::code().dockerfile(),
                comment_syntax: Some("# ".to_string()),
                ..Default::default()
            }
        } else if matches!(fname, "CMakeLists.txt") || matches!(fextension, "cmake") {
            Self {
                highlighter: Highlighter::code().cmake(),
                comment_syntax: Some("# ".to_string()),
                ..Default::default()
            }
        } else if matches!(fextension, "camkes") {
            Self {
                highlighter: Highlighter::code().camkes(),
                comment_syntax: Some("// ".to_string()),
                ..Default::default()
            }
        } else if matches!(fextension, "cherb") {
            Self {
                highlighter: Highlighter::code().cherb(),
                comment_syntax: Some("# ".to_string()),
                ..Default::default()
            }
        } else {
            Self {
                highlighter: Highlighter::code(),
                ..Default::default()
            }
        }
    }

    fn clike(highlighter: Highlighter) -> Self {
        Self {
            highlighter,
            comment_syntax: Some("// ".to_string()),
            delims: vec![('(', ')'), ('{', '}'), ('[', ']')],
            ..Default::default()
        }
    }

    fn pythonic(highlighter: Highlighter) -> Self {
        Self {
            highlighter,
            comment_syntax: Some("# ".to_string()),
            delims: vec![('(', ')'), ('{', '}'), ('[', ']')],
            ..Default::default()
        }
    }
}

impl Highlighter {
    pub fn code() -> Self {
        Self::default().git()
    }

    pub fn with_comment(self, regex: &str) -> Self {
        self.with_child_syntax(
            TokenKind::Comment,
            regex,
            Self::default()
                .url()
                .with(TokenKind::Important, r"\b[Tt][Oo][Dd][Oo]\b"),
        )
    }

    pub fn markdown(self) -> Self {
        self
            // Radio button
            .with(TokenKind::Macro, r"\[[[[:space:]]xX\-]\]")
            // Links
            .with_child_syntax(
                TokenKind::String,
                r"!?\[[^\]]*\](\([^\)]*\))?",
                Self::default().url(),
            )
            // Header
            .with(TokenKind::Title, r"^#+[[:space:]][^$]*$")
            // List item
            .with(TokenKind::Operator, r"^[[:space:]]?[\-([0-9]+[\)\.])]")
            // Bold
            .with(TokenKind::Bold, r"\*\*[^(\*\*)]*\*\*")
            // Italics
            .with(TokenKind::Italic, r"\*[^\*]*\*")
            // Code block
            .with(TokenKind::Operator, r"^```[^(^```)]*^```")
            // Inline code
            .with(TokenKind::Constant, r"`[^`$]*[`$]")
            // HTML
            .with(TokenKind::Macro, r"<[^<>]*>")
    }

    pub fn rust(self) -> Self {
        self
            // Both kinds of comments match multiple lines
            .with_child_syntax(TokenKind::Doc, r"\/\/[\/!][^\n]*$(\n[[:space:]]\/\/[\/!][^\n]*$)*", Self::default().url())
            .with_comment(r"\/\/[^$]*$(\n[[:space:]]\/\/[^$]*$)*")
            // Multi-line comment
            .with_comment(r"\/\*[^(\*\/)]*\*\/")
            .with(
                TokenKind::Keyword,
                r"\b[(async)(pub)(enum)(let)(self)(Self)(fn)(impl)(struct)(use)(if)(while)(for)(in)(loop)(mod)(match)(else)(break)(continue)(trait)(const)(static)(type)(as)(crate)(extern)(move)(ref)(return)(super)(unsafe)(use)(where)(dyn)(try)(gen)(macro_rules)(union)(raw)]\b",
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
            // "foo {fmt}"
            .with_child_syntax(
                TokenKind::String,
                r#""[(\\")[^"]]*""#,
                Highlighter::default().with(TokenKind::FmtString, r#"\{[^\{]%[(\{\{)(\}\})[^\}]]*\}[^\}]%"#)
            )
            // "foo" or b"foo" or r#"foo"#
            .with(TokenKind::String, r#"b?r?(#*)@("[(\\")[^("~)]]*("~))"#)
            // Characters
            .with(
                TokenKind::String,
                r#"b?'[(\\[nrt\\0(x[0-9A-Za-z][0-9A-Za-z])])[^']]'"#,
            )
            .with(
                TokenKind::Operator,
                r"[(&(mut)?)(\bmut\b)(\?)(\+=?)(\-=?)(\*=?)(\/=?)(\%=?)(!=?)(==?)(&&?=?)(\|\|?=?)(<<?=?)(>>?=?)(\.\.[\.=]?)\\\~\^:;,\@(=>?)]",
            )
            // Function/method call
            .with(TokenKind::Function, r"(\.)?\b[a-z_][A-Za-z0-9_]*\b[\(<]%")
            // Fields and methods: a.foo
            .with(TokenKind::Property, r"\.[a-z_][A-Za-z0-9_]*")
            // Paths: std::foo::bar
            .with(TokenKind::Property, r"[A-Za-z_][A-Za-z0-9_]*::")
            // Lifetimes
            .with(TokenKind::Special, r"'[a-z_][A-Za-z0-9_]*\b")
            .with(TokenKind::Ident, r"\b[a-z_][A-Za-z0-9_]*\b")
            .with(TokenKind::Number, r"\b[0-9][A-Za-z0-9_(\.[^\.])]*\b")
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
            .with_comment(r"\/\/[^$]*$(\n[[:space:]]\/\/[^$]*$)*")
            // Multi-line comment
            .with_comment(r"\/\*[^(\*\/)]*\*\/")
    }

    fn preprocessor(self) -> Self {
        self
            .with(TokenKind::MacroKeyword, r#"#[[:space:]]*\b[(ifdef)(ifndef)(if)(elif)(else)(endif)(include)(defined)(define)(undef)]\b"#)
            .with(TokenKind::String, r#"<[0-9A-Za-z\.\\\/]*>"#)
            .with(TokenKind::String, r#""[(\\")[^"]]*""#)
    }

    fn clike_preprocessor(self) -> Self {
        self.with_child_syntax(
            TokenKind::Macro,
            r"^[[:space:]]*#[(\\\n)[^$]]*$",
            Self::default().preprocessor(),
        )
    }

    pub fn generic_delimited(self) -> Self {
        self.with(TokenKind::String, r#""[(\\")[^"]]*""#)
            .with(TokenKind::Delimiter, r"[\{\}\(\)\[\]]")
            .with(TokenKind::Number, r"\b[0-9][A-Za-z0-9_\.]*\b")
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
                r"[(&)(\?)(\+\+)(\-\-)(\+=?)(\-=?)(\*=?)(\/=?)(\%=?)(!=?)(==?)(&&?=?)(\|\|?=?)(<<?=?)(>>?=?)(\.\.[\.=]?)\\\~\^:;,\@(=>?)]",
            )
            // Function/method call
            .with(TokenKind::Function, r"(\.)?\b[a-z_][A-Za-z0-9_]*\b[\(<]%")
            // Fields and methods: a.foo
            .with(TokenKind::Property, r"\.[A-Za-z_][A-Za-z0-9_]*")
            // Paths: std::foo::bar
            .with(TokenKind::Property, r"[A-Za-z_][A-Za-z0-9_]*::")
            .with(TokenKind::Ident, r"\b[a-z_][A-Za-z0-9_]*\b")
            .with(TokenKind::Number, r"\b[0-9][A-Za-z0-9_\.]*\b")
            .with(TokenKind::Delimiter, r"[\{\}\(\)\[\]]")
    }

    pub fn generic_clike(self) -> Self {
        self
            // Keywords
            .with(TokenKind::Keyword, r"\b[(var)(enum)(let)(this)(fn)(struct)(class)(import)(if)(while)(for)(in)(loop)(else)(break)(continue)(const)(static)(typedef)(type)(extern)(return)(async)(throw)(catch)(union)(auto)(namespace)(public)(private)(function)(func)(goto)(case)(default)(switch)(inline)(volatile)(do)]\b")
            // Primitives
            .with(TokenKind::Type, r"\b[(([(unsigned)(signed)][[:space:]])*u?int[0-9]*(_t)?)(float)(double)(bool)(char)(size_t)(void)]\b")
            .clike_comments()
            .clike()
    }

    pub fn cpp(self) -> Self {
        self
            // Dereferenced fields and methods: a->foo
            .with(TokenKind::Property, r"\->[A-Za-z_][A-Za-z0-9_]*")
            // Labels
            .with(TokenKind::Special, r"\b[a-z_][A-Za-z0-9_]*\b:")
            // Format string
            .with_child_syntax(
                TokenKind::String,
                r#""[(\\")[^"]]*""#,
                // Holy hell, C format specifiers are complicated
                Highlighter::default().with(TokenKind::FmtString, r#"\%[\-\+#0]*[[0-9]+\*]?(\.[[0-9]+\*])?[(hh)h(ll)lJztL]?[0-9]*[csdioxXufFeEaAgGnp]"#)
            )
            // types_like_this_t
            .with(TokenKind::Type, r"\b[a-z0-9(_[^t\b])]+_t\b")
            .generic_clike()
            .clike_preprocessor()
    }

    pub fn glsl(self) -> Self {
        self
            // Keywords
            .with(TokenKind::Keyword, r"\b[(struct)(if)(while)(for)(else)(break)(continue)(const)(return)(layout)(uniform)(set)(binding)(location)(inout)(in)(case)(switch)(default)]\b")
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
            .with(TokenKind::Doc, r"^[[:space:]]##[^$]*$")
            // Comments
            .with_comment(r"#[^$]*$")
            .clike()
    }

    pub fn tao(self) -> Self {
        self
            // Keywords
            .with(TokenKind::Keyword, r"\b[(data)(member)(def)(class)(type)(effect)(import)(handle)(with)(match)(if)(else)(for)(of)(let)(fn)(return)(in)(mod)(where)(when)(do)]\b")
            // Primitives
            .with(TokenKind::Type, r"\b[(Str)(Bool)(Nat)(Char)]\b")
            // Builtins
            .with(TokenKind::Macro, r"\b[(True)(False)]\b")
            // Doc comments
            .with(TokenKind::Doc, r"^[[:space:]]##[^$]*$")
            // Comments
            .with_comment(r"#[^$]*$")
            // Attributes
            .with(TokenKind::Attribute, r"\$!?\[[^\]]*\]")
            .clike()
    }

    pub fn toml(self) -> Self {
        self
            // Header
            .with(TokenKind::Doc, r#"^\[[^\n\]]*\]"#)
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
            .with_comment(r"#[^$]*$")
    }

    pub fn yaml(self) -> Self {
        self
            // Delimiters
            .with(TokenKind::Delimiter, r"[\{\}\(\)\[\]]")
            // Operators
            .with(TokenKind::Operator, r"[\-:]")
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
            .with_comment(r"#[^$]*$")
    }

    pub fn shell(self) -> Self {
        fn shell_string() -> Highlighter {
            Highlighter::default().with(TokenKind::FmtString, r#"\$\{[A-Za-z_][A-Za-z0-9_]*\}"#)
        }

        self.url()
            // Argument tag (like `--foo`)
            .with(TokenKind::Property, r"\-\-?[A-Za-z0-9_\-:\.\/]+\b")
            // Identifier/name/file
            .with(TokenKind::Ident, r"\b[A-Za-z0-9_\-:\.\/]+\b")
            // Operators
            .with(TokenKind::Operator, r"[=,:\@\+(\+\+)\+\\(&&)\|(\-\-)\-]")
            // Double-quoted strings
            .with_child_syntax(
                TokenKind::String,
                r#"b?"[(\\[nrt\\0(x[0-7A-Za-z][0-7A-Za-z])])[^"]]*""#,
                shell_string(),
            )
            // Single-quoted strings
            .with_child_syntax(
                TokenKind::String,
                r#"b?'[(\\[nrt\\0(x[0-7A-Za-z][0-7A-Za-z])])[^']]*'"#,
                shell_string(),
            )
            // Backtick-quoted strings
            .with_child_syntax(
                TokenKind::String,
                r#"b?`[(\\[nrt\\0(x[0-7A-Za-z][0-7A-Za-z])])[^`]]*`"#,
                shell_string(),
            )
            // Variables
            .with(TokenKind::Constant, r"\$\([A-Za-z_][A-Za-z0-9_\-]*\)")
            // Comments
            .with_comment(r"#[^$]*$")
    }

    pub fn makefile(self) -> Self {
        self
            // Keywords
            .with(
                TokenKind::Keyword,
                r"\b[(if(n)?[(eq)(def)]?)(endif)(else)]\b",
            )
            // Rules
            .with(TokenKind::Type, r"^[A-Za-z0-9_\-\.]*:")
            .shell()
    }

    pub fn dockerfile(self) -> Self {
        self
            // Keywords
            .with(
                TokenKind::Keyword,
                r"\b[(ADD)(ARG)(CMD)(COPY)(ENTRYPOINT)(ENV)(EXPOSE)(FROM)(AS)(HEALTHCHECK)(LABEL)(MAINTAINER)(ONBUILD)(RUN)(SHELL)(STOPSIGNAL)(USER)(VOLUME)(WORKDIR)(INCLUDE_ARGS)(INCLUDE_ENVS)(INCLUDE_LABELS)(INCLUDE)]\b",
            )
            .shell()
    }

    pub fn git(self) -> Self {
        self.with(
            TokenKind::MergeConflict,
            r"^[(<<<<<<<)(=======)(>>>>>>>)]( [^$]*)?$",
        )
    }

    pub fn url(self) -> Self {
        self
            // Automatically detect URLs
            .with(TokenKind::Url, r"\b(https?):\/\/[A-Za-z0-9_\-:\.\/#]+")
    }

    pub fn cherb(self) -> Self {
        self.generic_delimited()
            // Keywords
            .with(
                TokenKind::Keyword,
                r"\b[(BUILD)(IMPORT)(EXPORT)(RUN)(SOURCE)(OPTIONS?)(ENV)(USES?)]\b",
            )
            // Metavars
            .with(TokenKind::Macro, r"\$[A-Za-z_][A-Za-z0-9_]*")
            .shell()
    }

    pub fn cmake(self) -> Self {
        self
            // Keywords
            .with(TokenKind::Keyword, r"\b[(if)(else)(endif)]\b")
            .with(TokenKind::Function, r"\b[A-Za-z0-9_\-:\.\/]+\b\(%")
            // Variables
            .with(TokenKind::Constant, r"\$\([A-Za-z_][A-Za-z0-9_\-]*\)")
            .shell()
            .generic_delimited()
    }

    pub fn camkes(self) -> Self {
        self
            // Keywords
            .with(TokenKind::Keyword, r"\b[(import)(component)(include)(hardware)(dataport)(maybe)(emits)(configuration)(connection)(uses)(provides)(consumes)(has)(control)]\b")
            // Special types
            .with(TokenKind::Type, r"\b[(mutex)(semaphore)]\b")
            .clike_comments()
            .clike_preprocessor()
            .clike()
    }
}
