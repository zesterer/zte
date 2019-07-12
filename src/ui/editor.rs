use super::{
    Context,
    Element,
};
use crate::Canvas;

pub struct Editor;

impl Default for Editor {
    fn default() -> Self {
        Self
    }
}

impl Element for Editor {
    fn render(&self, ctx: Context, canvas: &mut impl Canvas) {
        canvas
            .with_bg(ctx.theme.editor_bg_color)
            .rectangle((0, 0), (-1, -1), ' ');
        canvas
            .with_bg(ctx.theme.frame_bg_color)
            .frame((0, 0), (-1, -1));
    }
}
