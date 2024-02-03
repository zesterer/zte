use std::{
    path::PathBuf,
    fs::DirEntry,
    time::SystemTime,
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
        let file_filter_lower = file_filter.to_lowercase();

        let now = SystemTime::now();

        let score_name = |file: &DirEntry| {
            const STARTS_WITH: i32 = 1000;
            const STARTS_WITH_LOWER: i32 = 800;
            const CONTAINS: i32 = 400;
            const CONTAINS_LOWER: i32 = 300;
            const HIDDEN: i32 = -200;
            const CHANGED: i32 = 200;
            const IS_DIR: i32 = 0;

            let fname = file.file_name().to_str().unwrap_or("").to_string();
            let fname_lower = fname.to_lowercase();

            0
                + file.metadata().map_or(false, |f| f.is_dir()) as i32 * IS_DIR
                + fname.starts_with(&file_filter) as i32 * STARTS_WITH
                + fname_lower.starts_with(&file_filter_lower) as i32 * STARTS_WITH_LOWER
                + fname.contains(&file_filter) as i32 * CONTAINS
                + fname_lower.contains(&file_filter_lower) as i32 * CONTAINS_LOWER
                + fname.to_lowercase().contains(&file_filter.to_lowercase()) as i32 * CONTAINS
                + fname.starts_with(".") as i32 * HIDDEN
                + file.metadata()
                    .and_then(|m| m.accessed())
                    .map_or(0, |time| (1.0 / (now.duration_since(time).map(|d| d.as_secs_f32()).unwrap_or(0.0) + 1.0)) as i32 * CHANGED)
        };

        self.listings = match self.path.read_dir() {
            Ok(dir) => {
                let mut entries = dir
                    .filter_map(|entry| entry.ok())
                    .filter(|entry| entry
                        .path()
                        .file_name()
                        .map(|f| f.to_str().unwrap_or("").to_lowercase().contains(&file_filter_lower))
                        .unwrap_or(false))
                    .collect::<Vec<_>>();
                if file_filter.is_empty() {
                    entries.sort_by_cached_key(|e| e.file_name().to_str().unwrap_or("").to_string());
                } else {
                    entries.sort_by_cached_key(|e| -score_name(e));
                }
                Some((0, entries))
            },
            Err(_) => None,
        };
    }
}

impl Element for Opener {
    type Response = Result<(), Event>;

    fn handle(&mut self, ctx: &mut Context, event: Event) -> Self::Response {
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
                self.prompt.handle(ctx, event)?;
                self.update_listings();
            },
        }
        Ok(())
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
            sz.w.saturating_sub(sz.w / 2),
            sz.h.saturating_sub(sz.h / 2),
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
            1 + path_text.chars().count(),
            0,
            canvas.size().w,
            canvas.size().h,
        )), active);

        // File listings
        let mut canvas = canvas.window(Rect::new(
            0,
            1,
            canvas.size().w,
            canvas.size().h.saturating_sub(1),
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
                            NumberPrefix::Standalone(bytes) => format!("{}  B", bytes),
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
