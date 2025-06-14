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
            top_left: '╭',
            top_right: '╮',
            bottom_left: '╰',
            bottom_right: '╯',
            join_left: '├',
            join_right: '┤',
            fg: Color::AnsiValue(244),
        }
    }
}

pub struct Theme {
    pub ui_bg: Color,
    pub select_bg: Color,
    pub unfocus_select_bg: Color,
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
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            ui_bg: Color::AnsiValue(235),
            select_bg: Color::AnsiValue(23),
            unfocus_select_bg: Color::AnsiValue(240),
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
            hl_token_ident: Color::AnsiValue(187),
            hl_token_keyword: Color::AnsiValue(46),
            hl_token_number: Color::AnsiValue(45),
            hl_token_type: Color::AnsiValue(203),
        }
    }
}

impl Theme {
    pub fn token_color(&self, token: TokenKind) -> Color {
        match token {
            TokenKind::Whitespace => self.hl_token_whitespace,
            TokenKind::Ident => self.hl_token_ident,
            TokenKind::Keyword => self.hl_token_keyword,
            TokenKind::Number => self.hl_token_number,
            TokenKind::Type => self.hl_token_type,
        }
    }
}
