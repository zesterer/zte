use std::{
    path::PathBuf,
    fs::DirEntry,
};
use vek::*;
use number_prefix::NumberPrefix;
use crate::{
    Canvas,
    Event,
    Dir,
    display::Color,
};
use super::{
    Context,
    Element,
    Prompt,
};

pub struct Opener {
    prompt: Prompt,
    path: PathBuf,
    listings: Option<(usize, Vec<DirEntry>)>,
}

impl Opener {
    pub fn new(ctx: &mut Context) -> Self {
        let mut this = Self {
            prompt: Prompt::default(),
            path: ctx.state
                .get_shared_buffer(ctx.active_buffer)
                .and_then(|buf| buf.path
                    .as_ref()
                    .and_then(|p| p.parent())
                    .map(|p| p.to_owned()))
                .or_else(|| std::env::current_dir().ok())
                .unwrap_or_default(),
            listings: None,
        };

        this.update_listings();

        this
    }

    pub fn update_listings(&mut self) {
        let file_filter = self.prompt.get_text();
        self.listings = match self.path.read_dir() {
            Ok(dir) => {
                let mut entries = dir
                    .filter_map(|entry| entry.ok())
                    .filter(|entry| entry
                        .path()
                        .file_stem()
                        .map(|f| f.to_str().unwrap().contains(&file_filter))
                        .unwrap_or(false))
                    .collect::<Vec<_>>();
                entries.sort_by_key(|e| e.file_name().to_str().map(|s| s.to_string()));
                Some((0, entries))
            },
            Err(_) => None,
        };
    }
}

impl Element for Opener {
    type Response = ();

    fn handle(&mut self, ctx: &mut Context, event: Event) {
        match event {
            Event::Insert('/') => {
                self.path.push(format!("{}/", self.prompt.get_text()));
                self.update_listings();
                self.prompt = Prompt::default();
            },
            Event::Insert(c @ ('\n' | '\t')) => {
                if let Some((selected_idx, listings)) = &mut self.listings {
                    if *selected_idx < listings.len() {
                        let entry = &listings[*selected_idx];
                        let is_file = entry.file_type().map(|ft| ft.is_file()).unwrap_or(false);

                        self.path.push(entry.file_name().to_str().unwrap());

                        if is_file {
                            ctx.secondary_events.push_back(Event::CloseMenu);
                            ctx.secondary_events.push_back(Event::OpenFile(self.path.clone()));
                        }

                        self.prompt = Prompt::default();
                        self.update_listings();
                    } else {
                        ctx.secondary_events.push_back(Event::CloseMenu);
                        self.path.push(self.prompt.get_text());
                        ctx.secondary_events.push_back(Event::NewFile(self.path.clone()));
                    }
                }
            },
            Event::CursorMove(Dir::Up, _) => {
                let prompt_len = self.prompt.get_text().len();
                self.listings.as_mut().map(|(selected_idx, listings)| {
                    let select_len = listings.len() + if prompt_len == 0 { 0 } else { 1 };
                    *selected_idx = (*selected_idx + select_len.saturating_sub(1)) % select_len;
                });
            },
            Event::CursorMove(Dir::Down, _) => {
                let prompt_len = self.prompt.get_text().len();
                self.listings.as_mut().map(|(selected_idx, listings)| {
                    let select_len = listings.len() + if prompt_len == 0 { 0 } else { 1 };
                    *selected_idx = (*selected_idx + 1) % select_len;
                });
            },
            Event::Backspace if self.prompt.get_text().len() == 0 => {
                self.path.pop();
                self.update_listings();
            },
            event => {
                self.prompt.handle(ctx, event);
                self.update_listings();
            },
        }
    }

    fn update(&mut self, ctx: &mut Context, canvas: &mut impl Canvas, active: bool) {
        self.prompt.set_fg_color(if self.listings.as_ref().map(|(_, entries)| entries.is_empty()).unwrap_or(true) {
            ctx.theme.create_color
        } else {
            Color::Reset
        });

        let prompt_len = self.prompt.get_text().len();
        self.listings
            .as_mut()
            .map(|(idx, listings)| {
                if prompt_len == 0 {
                    *idx = *idx % listings.len();
                }
            });

        self.prompt.update(ctx, canvas, active);
    }

    fn render(&self, ctx: &mut Context, canvas: &mut impl Canvas, active: bool) {
        let sz = canvas.size();
        let mut canvas = canvas.window(Rect::new(
            sz.w / 4,
            sz.h / 4,
            sz.w - sz.w / 2,
            sz.h - sz.h / 2,
        ));

        // Frame
        let sz = canvas.size();
        canvas.rectangle(Vec2::zero(), sz, ' '.into());
        canvas.frame();

        let title = format!("[Open File]");
        canvas.write_str(Vec2::new((sz.w.saturating_sub(title.len())) / 2, 0), &title);

        const DIR_COLOR: Color = Color::Rgb(Rgb::new(255, 200, 100));

        // Prompt
        let mut path_text = format!("{}", self.path.display());
        if path_text.chars().nth(path_text.len() - 1).map(|c| c != '/').unwrap_or(false) {
            path_text.push('/');
        }
        let mut canvas = canvas.window(Rect::new(1, 1, canvas.size().w - 2, canvas.size().h - 2));
        // Render most of path
        canvas.with_fg(DIR_COLOR).write_str(Vec2::new(1, 0), &path_text);
        // Render prompt (last path element)
        self.prompt.render(ctx, &mut canvas.window(Rect::new(
            1 + path_text.len(),
            0,
            canvas.size().w,
            canvas.size().h,
        )), active);

        // File listings
        let mut canvas = canvas.window(Rect::new(
            0,
            1,
            canvas.size().w,
            canvas.size().h - 1,
        ));

        if let Some((selected_idx, listings)) = &self.listings {
            for (y, (i, entry)) in listings
                .iter()
                .map(Some)
                .chain(if self.prompt.get_text().len() == 0 {
                    None
                } else {
                    Some(None)
                }.into_iter())
                .enumerate()
                .skip(selected_idx.saturating_sub(canvas.size().h.saturating_sub(1)))
                .take(canvas.size().h)
                .enumerate()
            {
                let bg_color = if i == *selected_idx {
                    ctx.theme.selection_color
                } else {
                    Color::Reset
                };
                if let Some(entry) = entry {
                    if entry.file_type().map(|ft| ft.is_file()).unwrap_or(true) {
                        canvas
                            .with_fg(Color::Rgb(Rgb::new(255, 255, 255)))
                            .with_bg(bg_color)
                            .write_str(Vec2::new(1, y), &format!("{:<48}", entry.file_name().to_str().unwrap_or("!!!")));
                    } else {
                        canvas
                            .with_fg(DIR_COLOR)
                            .with_bg(bg_color)
                            .write_str(Vec2::new(1, y), &format!("{:<47}", &format!("{}/", entry.file_name().to_str().unwrap_or("!!!"))));
                    }

                    if let Ok(meta) = entry.metadata() {
                        let file_size = match NumberPrefix::decimal(meta.len() as f64) {
                            NumberPrefix::Standalone(bytes) => format!("{} B", bytes),
                            NumberPrefix::Prefixed(prefix, n) => format!("{:.1} {}B", n, prefix),
                        };

                        let perms = if meta.permissions().readonly() { "r" } else { "rw" };

                        let file_type = if meta.file_type().is_file() {
                            "file"
                        } else if meta.file_type().is_dir() {
                            "dir"
                        } else {
                            "symlink"
                        };

                        let desc = format!("{:>8} {:>6} {:>8}", file_size, perms, file_type);

                        canvas
                            .with_fg(ctx.theme.subtle_color)
                            .write_str(Vec2::new(49, y), &desc);
                    }
                } else {
                    canvas
                        .with_fg(ctx.theme.create_color)
                        .with_bg(bg_color)
                        .write_str(Vec2::new(1, y), &format!("{:<48}[new]", self.prompt.get_text()));
                }
            }
        } else {
            canvas.write_str(Vec2::zero(), "<Invalid directory>");
        }
    }
}
