use std::ops::Range;

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

const PRIMITIVES: [&str; 16] = ["usize", "isize", "u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32", "i64", "i128", "f32", "f64", "str", "bool"];

impl From<String> for Highlights {
    fn from(code: String) -> Self {
        enum State {
            Default,
            Number,
            Word,
            String,
            Symbol(char),
            LineComment,
            MultiComment(char),
        }

        let mut chars = code.chars().enumerate();
        let mut state = State::Default;
        let mut regions = Vec::new();
        let mut start = 0;

        loop {
            let (i, c) = chars.clone().next().unwrap_or((0, '\0'));
            let mut wait = false;
            match state {
                State::Default => match c {
                    '\0' => break,
                    '"' => {
                        state = State::String;
                        start = i;
                    },
                    c if c.is_whitespace() => {},
                    c if c.is_alphabetic() || c == '_' => {
                        state = State::Word;
                        start = i;
                    },
                    c if c.is_numeric() => {
                        state = State::Number;
                        start = i;
                    },
                    c if c.is_ascii_punctuation() && c != '"' => {
                        state = State::Symbol(c);
                        start = i;
                    },
                    c => {},
                },
                State::Number => match c {
                    c if c.is_alphanumeric() || c == '_' || c == '.' => {},
                    c => {
                        regions.push((start..i, Region::Numeric));
                        wait = true;
                        state = State::Default;
                    },
                },
                State::Word => match c {
                    c if c.is_alphanumeric() || c == '_' => {},
                    c => {
                        regions.push((start..i, match &code[start..i] {
                            "struct" => Region::Keyword,
                            "enum" => Region::Keyword,
                            "use" => Region::Keyword,
                            "match" => Region::Keyword,
                            "if" => Region::Keyword,
                            "else" => Region::Keyword,
                            "loop" => Region::Keyword,
                            "while" => Region::Keyword,
                            "let" => Region::Keyword,
                            "fn" => Region::Keyword,
                            "pub" => Region::Keyword,
                            "continue" => Region::Keyword,
                            "break" => Region::Keyword,
                            "return" => Region::Keyword,
                            "as" => Region::Keyword,
                            "const" => Region::Keyword,
                            "crate" => Region::Keyword,
                            "extern" => Region::Keyword,
                            "true" => Region::Keyword,
                            "false" => Region::Keyword,
                            "impl" => Region::Keyword,
                            "for" => Region::Keyword,
                            "in" => Region::Keyword,
                            "mod" => Region::Keyword,
                            "move" => Region::Keyword,
                            "mut" => Region::Keyword,
                            "ref" => Region::Keyword,
                            "self" => Region::Keyword,
                            "Self" => Region::Keyword,
                            "static" => Region::Keyword,
                            "trait" => Region::Keyword,
                            "type" => Region::Keyword,
                            "unsafe" => Region::Keyword,
                            "where" => Region::Keyword,
                            s if PRIMITIVES.contains(&s) => Region::Primitive,
                            _ => Region::Normal,
                        }));
                        wait = true;
                        state = State::Default;
                    },
                },
                State::String => match c {
                    c if c == '"' || c == '\0' => {
                        regions.push((start..i + 1, Region::String));
                        state = State::Default;
                    },
                    c => {},
                },
                State::Symbol(last) => match c {
                    '/' if last == '/' => {
                        state = State::LineComment;
                    },
                    '*' if last == '/' => {
                        state = State::MultiComment(c);
                    },
                    c if c.is_ascii_punctuation() && c != '"' => {},
                    c => {
                        regions.push((start..i, Region::Symbol));
                        wait = true;
                        state = State::Default;
                    },
                },
                State::LineComment => match c {
                    '\n' => {
                        regions.push((start..i, Region::LineComment));
                        state = State::Default;
                    },
                    c => {},
                },
                State::MultiComment(last) => match c {
                    c if c == '\0' || (c == '/' && last == '*') => {
                        regions.push((start..i, Region::LineComment));
                        state = State::Default;
                    },
                    c => {},
                },
            }

            if !wait {
                chars.next();
            }
        }

        Self { regions }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Region {
    Normal,
    Keyword,
    LineComment,
    MultiComment,
    Special,
    Primitive,
    Symbol,
    Bracket,
    Numeric,
    String,
}