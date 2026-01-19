use crate::{Color, highlight::TokenKind};

pub struct BorderTheme {
    pub left: char,
    pub right: char,
    pub top: char,
    pub bottom: char,
    pub top_left: char,
    pub top_right: char,
    pub bottom_left: char,
    pub bottom_right: char,
    pub join_left: char,
    pub join_right: char,
    pub fg: Color,
}

impl Default for BorderTheme {
    fn default() -> Self {
        Self {
            left: '│',
            right: '│',
            top: '─',
            bottom: '─',
            top_left: '┌',     //'╭',
            top_right: '┐',    //'╮',
            bottom_left: '└',  //'╰',
            bottom_right: '┘', //'╯',
            join_left: '├',
            join_right: '┤',
            fg: Color::AnsiValue(244),
        }
    }
}

#[derive(Copy, Clone, Default)]
pub struct CellTheme {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
}

impl CellTheme {
    pub fn fg(mut self, fg: Color) -> Self {
        self.fg = Some(fg);
        self
    }

    pub fn bg(mut self, bg: Color) -> Self {
        self.bg = Some(bg);
        self
    }
}

pub struct Theme {
    pub select: CellTheme,
    pub line_select: CellTheme,
    pub unfocus_select: CellTheme,
    pub search_result: CellTheme,
    pub margin: CellTheme,
    pub border: BorderTheme,
    pub focus_border: BorderTheme,
    pub whitespace: CellTheme,
    pub option_dir: CellTheme,
    pub option_file: CellTheme,
    pub option_new: CellTheme,

    pub hl_token_whitespace: CellTheme,
    pub hl_token_ident: CellTheme,
    pub hl_token_keyword: CellTheme,
    pub hl_token_number: CellTheme,
    pub hl_token_type: CellTheme,
    pub hl_token_comment: CellTheme,
    pub hl_token_operator: CellTheme,
    pub hl_token_delimiter: CellTheme,
    pub hl_token_doc: CellTheme,
    pub hl_token_attribute: CellTheme,
    pub hl_token_property: CellTheme,
    pub hl_token_macro: CellTheme,
    pub hl_token_string: CellTheme,
    pub hl_token_special: CellTheme,
    pub hl_token_constant: CellTheme,
    pub hl_token_function: CellTheme,

    pub hl_merge_conflict: CellTheme,
    pub hl_url: CellTheme,
    pub hl_important: CellTheme,
}

impl Default for Theme {
    fn default() -> Self {
        let none = CellTheme::default();
        let fg = |fg| CellTheme::default().fg(fg);
        let bg = |bg| CellTheme::default().bg(bg);

        Self {
            select: bg(Color::AnsiValue(8)),
            line_select: bg(Color::AnsiValue(238)),
            unfocus_select: bg(Color::AnsiValue(238)),
            search_result: bg(Color::AnsiValue(60)),
            margin: fg(Color::AnsiValue(245)),
            border: BorderTheme::default(),
            focus_border: BorderTheme {
                fg: Color::White,
                ..BorderTheme::default()
            },
            whitespace: fg(Color::AnsiValue(245)),
            option_dir: fg(Color::AnsiValue(178)),
            option_file: none,
            option_new: fg(Color::AnsiValue(148)),

            hl_token_whitespace: none,
            hl_token_ident: fg(Color::AnsiValue(15)),
            hl_token_keyword: fg(Color::AnsiValue(112)),
            hl_token_number: fg(Color::AnsiValue(45)),
            hl_token_type: fg(Color::AnsiValue(210)),
            hl_token_comment: fg(Color::AnsiValue(145)),
            hl_token_operator: fg(Color::AnsiValue(111)),
            hl_token_delimiter: fg(Color::AnsiValue(37)),
            hl_token_doc: fg(Color::AnsiValue(180)),
            hl_token_attribute: fg(Color::AnsiValue(146)),
            hl_token_property: fg(Color::AnsiValue(152)),
            hl_token_macro: fg(Color::AnsiValue(117)),
            hl_token_string: fg(Color::AnsiValue(179)),
            hl_token_special: fg(Color::AnsiValue(160)),
            hl_token_constant: fg(Color::AnsiValue(81)),
            hl_token_function: fg(Color::AnsiValue(122)),

            hl_merge_conflict: fg(Color::AnsiValue(124)),
            hl_url: fg(Color::AnsiValue(123)),
            hl_important: none.bg(Color::AnsiValue(52)),
        }
    }
}

impl Theme {
    pub fn token_theme(&self, token: TokenKind) -> CellTheme {
        match token {
            TokenKind::Whitespace => self.hl_token_whitespace,
            TokenKind::Ident => self.hl_token_ident,
            TokenKind::Keyword => self.hl_token_keyword,
            TokenKind::Number => self.hl_token_number,
            TokenKind::Type => self.hl_token_type,
            TokenKind::Comment => self.hl_token_comment,
            TokenKind::Operator => self.hl_token_operator,
            TokenKind::Delimiter => self.hl_token_delimiter,
            TokenKind::Doc => self.hl_token_doc,
            TokenKind::Attribute => self.hl_token_attribute,
            TokenKind::Property => self.hl_token_property,
            TokenKind::Macro => self.hl_token_macro,
            TokenKind::String => self.hl_token_string,
            TokenKind::Special => self.hl_token_special,
            TokenKind::Constant => self.hl_token_constant,
            TokenKind::Function => self.hl_token_function,
            TokenKind::MergeConflict => self.hl_merge_conflict,
            TokenKind::Url => self.hl_url,
            TokenKind::Important => self.hl_important,
        }
    }
}
