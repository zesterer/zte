use vek::*;
use crate::{
    Canvas,
    Event,
    Dir,
    BufferHandle,
    Color,
};
use super::{
    Context,
    Element,
    Prompt,
};
pub struct List<T> {
    entries: Vec<T>,
    priorities: Vec<usize>,
    selected: Option<usize>,
}

impl<T> List<T> {
    pub fn new(entries: impl IntoIterator<Item = T>) -> Self {
        let entries = entries.into_iter().collect::<Vec<_>>();
        Self {
            priorities: (0..entries.len()).collect(),
            entries,
            selected: Some(0),
        }
    }
    
    pub fn update(&mut self, mut score: impl FnMut(&T) -> Option<i32>, reset: bool) {
        let old_selected = self.selected.map(|s| self.priorities[s]);

        let entries_score = (0..self.entries.len()).map(|i| score(&self.entries[i])).collect::<Vec<_>>();
        self.priorities.clear();
        self.priorities.extend((0..self.entries.len()).filter(|i| entries_score[*i].is_some()));
        self.priorities.sort_by_key(|i| std::cmp::Reverse(entries_score[*i]));
        
        self.selected = if self.priorities.is_empty() {
            None
        } else if reset {
            Some(0)
        } else {
            Some(self.priorities.iter().enumerate().find(|(_, x)| Some(**x) == old_selected).map(|(i, _)| i).unwrap_or(0))
        };
    }
    
    pub fn move_by(&mut self, r: isize) {
        if let Some(selected) = &mut self.selected {
            *selected = (*selected as isize + self.priorities.len() as isize + r) as usize % self.priorities.len();
        }
    }
    
    pub fn elements(&self) -> impl ExactSizeIterator<Item = &T> { self.priorities.iter().map(move |i| &self.entries[*i]) }
    
    pub fn selected_idx(&self) -> Option<usize> { self.selected }
    pub fn selected(&self) -> Option<&T> { Some(&self.entries[self.priorities[self.selected?]]) }
}

pub struct Switcher {
    prompt: Prompt,
    prev_buffer: BufferHandle,
    
    recent: List<usize>,
}

impl Switcher {
    pub fn new(ctx: &mut Context, prev_buffer: BufferHandle) -> Self {
        Self {
            prev_buffer,
            prompt: Prompt::default(),
            recent: List::new(0..ctx.state.recent_buffers().count()),
        }
    }

    pub fn cancel(self, ctx: &mut Context) {
        ctx.secondary_events.push_back(Event::SwitchBuffer(self.prev_buffer));
    }
}

impl Element for Switcher {
    type Response = Result<(), Event>;

    fn handle(&mut self, ctx: &mut Context, event: Event) -> Self::Response {
        let recent_count = ctx.state.recent_buffers().len();
        let last_selected = self.recent.selected().copied();
        match event {
            Event::CursorMove(Dir::Up, _) => self.recent.move_by(-1),
            Event::CursorMove(Dir::Down, _) => self.recent.move_by(1),
            Event::Insert('\n') => ctx.secondary_events.push_back(Event::CloseMenu),
            event => {
                let old_prompt = self.prompt.get_text();
                self.prompt.handle(ctx, event)?;
                
                let prompt = self.prompt.get_text();
                let prompt_lower = prompt.to_lowercase();
                
                let handles = ctx.state.recent_buffers().cloned().collect::<Vec<_>>();
                self.recent.update(|i| {
                    let buf = ctx.state.get_shared_buffer(handles[*i].buffer_id).unwrap();
                    
                    let title = buf.title();
                    let title_lower = title.to_lowercase();
                    
                    const STARTS_WITH: i32 = 8;
                    const STARTS_WITH_LOWER: i32 = 4;
                    const CONTAINS: i32 = 2;
                    const CONTAINS_LOWER: i32 = 1;
                    if title_lower.contains(&prompt_lower) {
                        Some(CONTAINS_LOWER
                            + title.contains(&prompt) as i32 * CONTAINS
                            + title.starts_with(&prompt) as i32 * STARTS_WITH
                            + title_lower.starts_with(&prompt_lower) as i32 * STARTS_WITH_LOWER)
                    } else {
                        None
                    }
                }, old_prompt != prompt);
            },
        }
        
        if self.recent.selected().copied() != last_selected {
            if let Some(selected) = self.recent.selected() {
                ctx.secondary_events.push_back(Event::SwitchBuffer({
                    let old_handle = ctx.state
                        .recent_buffers()
                        .nth(*selected)
                        .unwrap()
                        .clone();
                    ctx.state.duplicate_handle(&old_handle).unwrap()
                }));
            }
        }
        
        Ok(())
    }

    fn update(&mut self, ctx: &mut Context, canvas: &mut impl Canvas, active: bool) {
        self.prompt.set_fg_color(if self.recent.elements().len() == 0 {
            ctx.theme.invalid_color
        } else {
            Color::Rgb(Rgb::new(255, 255, 255))
        });
    }

    fn render(&self, ctx: &mut Context, canvas: &mut impl Canvas, active: bool) {
        let sz = canvas.size();
        let mut canvas = canvas.window(Rect::new(
            sz.w / 4,
            sz.h / 4,
            sz.w.saturating_sub(sz.w / 2),
            sz.h.saturating_sub(sz.h / 2),
        ));

        // Frame
        let sz = canvas.size();
        canvas.rectangle(Vec2::zero(), sz, ' '.into());
        canvas.frame();
        
        self.prompt.render(ctx, &mut canvas.window(Rect::new(
            2,
            1,
            canvas.size().w.saturating_sub(3),
            canvas.size().h,
        )), active);

        let title = format!("[Recent Buffers]");
        canvas.write_str(Vec2::new((sz.w.saturating_sub(title.len())) / 2, 0), &title);

        // Entries
        let mut canvas = canvas.window(Rect::new(
            1,
            2,
            canvas.size().w.saturating_sub(2),
            canvas.size().h.saturating_sub(3),
        ));

        let handles = ctx.state.recent_buffers().cloned().collect::<Vec<_>>();
        for (idx, i) in self.recent.elements().enumerate().take(canvas.size().h) {
            let y = idx;
            
            let bg_color = if Some(idx) == self.recent.selected_idx() {
                ctx.theme.selection_color
            } else {
                Color::Reset
            };

            let buf = ctx.state.get_shared_buffer(handles[*i].buffer_id).unwrap();
            
            canvas
                .with_fg(Color::Rgb(Rgb::new(255, 255, 255)))
                .with_bg(bg_color)
                .write_str(Vec2::new(1, y), &format!("{:<48}", buf.title()));
        }
    }
}
