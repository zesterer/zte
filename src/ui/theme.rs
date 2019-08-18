use vek::*;
//use serde_derive::{Serialize, Deserialize};
use crate::display::Color;

#[derive(Clone, Debug)]//, Serialize, Deserialize)]
pub struct Theme {
    pub editor_bg_color: Color,
    pub frame_bg_color: Color,
    pub margin_color: Color,
    pub line_number_color: Color,
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
