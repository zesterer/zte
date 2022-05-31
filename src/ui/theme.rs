use vek::*;
//use serde_derive::{Serialize, Deserialize};
use crate::{
    display::Color,
    buffer::highlight::Region,
};

#[derive(Clone, Debug)]//, Serialize, Deserialize)]
pub struct Theme {
    pub editor_bg_color: Color,
    pub frame_bg_color: Color,
    pub margin_color: Color,
    pub line_number_color: Color,
    pub scrollbar_color: Color,
    pub scrollpad_color: Color,
    pub selection_color: Color,
    pub create_color: Color,
    pub subtle_color: Color,
    pub subtle_bg_color: Color,
}

impl Theme {
    pub fn get_highlight_color(&self, region: Region) -> Color {
        match region {
            Region::Numeric => Color::Rgb(Rgb::new(255, 100, 200)),
            Region::Keyword => Color::Rgb(Rgb::new(50, 200, 100)),
            Region::String => Color::Rgb(Rgb::new(255, 200, 50)),
            Region::LineComment => Color::Rgb(Rgb::gray(120)),
            Region::MultiComment => Color::Rgb(Rgb::gray(180)),
            Region::Symbol => Color::Rgb(Rgb::new(0, 150, 255)),
            Region::Label => Color::Rgb(Rgb::new(255, 0, 0)),
            Region::Macro => Color::Rgb(Rgb::new(0, 255, 230)),
            Region::Type => Color::Rgb(Rgb::new(255, 100, 0)),
            Region::Constant => Color::Rgb(Rgb::new(225, 200, 255)),
            Region::Path => Color::Rgb(Rgb::new(225, 255, 200)),
            Region::Error => Color::Rgb(Rgb::new(225, 0, 0)),
            Region::Warning => Color::Rgb(Rgb::new(255, 180, 50)),
            Region::Info => Color::Rgb(Rgb::new(0, 200, 75)),
            _ => Color::Reset,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            editor_bg_color: Color::Reset,
            frame_bg_color: Color::Reset,
            margin_color: Color::Reset,
            line_number_color: Color::Rgb(Rgb::gray(100)),
            scrollbar_color: Color::Rgb(Rgb::gray(70)),
            scrollpad_color: Color::Rgb(Rgb::gray(175)),
            selection_color: Color::Rgb(Rgb::new(0, 100, 80)),
            create_color: Color::Rgb(Rgb::new(100, 255, 0)),
            subtle_color: Color::Rgb(Rgb::gray(150)),
            subtle_bg_color: Color::Rgb(Rgb::gray(65)),
        }
    }
}
