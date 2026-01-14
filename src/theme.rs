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

pub struct Theme {
    pub ui_bg: Color,
    pub select_bg: Color,
    pub line_select_bg: Color,
    pub unfocus_select_bg: Color,
    pub search_result_bg: Color,
    pub margin_bg: Color,
    pub margin_line_num: Color,
    pub border: BorderTheme,
    pub focus_border: BorderTheme,
    pub text: Color,
    pub whitespace: Color,
    pub option_dir: Color,
    pub option_file: Color,
    pub option_new: Color,

    pub hl_token_whitespace: Color,
    pub hl_token_ident: Color,
    pub hl_token_keyword: Color,
    pub hl_token_number: Color,
    pub hl_token_type: Color,
    pub hl_token_comment: Color,
    pub hl_token_operator: Color,
    pub hl_token_delimiter: Color,
    pub hl_token_doc: Color,
    pub hl_token_attribute: Color,
    pub hl_token_property: Color,
    pub hl_token_macro: Color,
    pub hl_token_string: Color,
    pub hl_token_special: Color,
    pub hl_token_constant: Color,
    pub hl_token_function: Color,

    pub hl_merge_conflict: Color,
    pub hl_url: Color,

    pub hl_matching_delimiter: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            ui_bg: Color::AnsiValue(235),
            select_bg: Color::AnsiValue(8),
            line_select_bg: Color::AnsiValue(238),
            unfocus_select_bg: Color::AnsiValue(238),
            search_result_bg: Color::AnsiValue(60),
            margin_bg: Color::Reset,
            margin_line_num: Color::AnsiValue(245),
            border: BorderTheme::default(),
            focus_border: BorderTheme {
                fg: Color::White,
                ..BorderTheme::default()
            },
            text: Color::Reset,
            whitespace: Color::AnsiValue(245),
            option_dir: Color::AnsiValue(178),
            option_file: Color::Reset,
            option_new: Color::AnsiValue(148),

            hl_token_whitespace: Color::Reset,
            hl_token_ident: Color::AnsiValue(15),
            hl_token_keyword: Color::AnsiValue(112),
            hl_token_number: Color::AnsiValue(45),
            hl_token_type: Color::AnsiValue(210),
            hl_token_comment: Color::AnsiValue(145),
            hl_token_operator: Color::AnsiValue(111),
            hl_token_delimiter: Color::AnsiValue(37),
            hl_token_doc: Color::AnsiValue(180),
            hl_token_attribute: Color::AnsiValue(146),
            hl_token_property: Color::AnsiValue(152),
            hl_token_macro: Color::AnsiValue(117),
            hl_token_string: Color::AnsiValue(179),
            hl_token_special: Color::AnsiValue(160),
            hl_token_constant: Color::AnsiValue(81),
            hl_token_function: Color::AnsiValue(122),

            hl_merge_conflict: Color::AnsiValue(124),
            hl_url: Color::AnsiValue(123),

            hl_matching_delimiter: Color::AnsiValue(123),
        }
    }
}

impl Theme {
    pub fn token_color(&self, token: TokenKind) -> (Color, Option<Color>) {
        let fg = match token {
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
            TokenKind::MergeConflict => self.text,
            TokenKind::Url => self.hl_url,
        };
        let bg = match token {
            TokenKind::MergeConflict => Some(self.hl_merge_conflict),
            _ => None,
        };
        (fg, bg)
    }
}
