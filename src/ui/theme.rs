use serde_derive::{Serialize, Deserialize};
use crate::display::Color;

#[derive(Clone, Debug)]//, Serialize, Deserialize)]
pub struct Theme {
    pub editor_bg_color: Color,
    pub frame_bg_color: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            editor_bg_color: Color::Red,
            frame_bg_color: Color::Red,
        }
    }
}
