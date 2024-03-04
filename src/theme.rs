use crate::Color;

pub struct BorderTheme {
    pub left: char,
    pub right: char,
    pub top: char,
    pub bottom: char,
    pub top_left: char,
    pub top_right: char,
    pub bottom_left: char,
    pub bottom_right: char,
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
        }
    }
}

pub struct Theme {
    pub ui_bg: Color,
    pub status_bg: Color,
    pub border: BorderTheme,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            ui_bg: Color::AnsiValue(235),
            status_bg: Color::AnsiValue(23),
            border: BorderTheme::default(),
        }
    }
}
