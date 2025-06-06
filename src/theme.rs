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
    pub margin_bg: Color,
    pub margin_line_num: Color,
    pub border: BorderTheme,
    pub focus_border: BorderTheme,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            ui_bg: Color::AnsiValue(235),
            select_bg: Color::AnsiValue(23),
            margin_bg: Color::Reset,
            margin_line_num: Color::AnsiValue(245),
            border: BorderTheme::default(),
            focus_border: BorderTheme {
                fg: Color::White,
                ..BorderTheme::default()
            },
        }
    }
}
