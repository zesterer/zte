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
            Region::Primitive => Color::Rgb(Rgb::new(255, 100, 0)),
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
        }
    }
}
