use super::*;
use crate::state::{Buffer, CursorId};
use std::fs;

pub struct Searcher {
    options: Options<SearchResult>,
    path: PathBuf,
    needle: String,
    // Filter
    buffer: Buffer,
    cursor_id: CursorId,
    input: Input,
    preview: Option<(Buffer, CursorId, Input, SearchResult)>,
}

impl Searcher {
    pub fn new(mut path: PathBuf, needle: String) -> Self {
        let path = loop {
            if let Ok(mut entries) = fs::read_dir(&path)
                && entries.any(|e| {
                    e.map_or(false, |e| {
                        e.file_name() == ".git" && e.file_type().map_or(false, |t| t.is_dir())
                    })
                })
            {
                break path;
            } else if !path.pop() {
                break std::env::current_dir().expect("No cwd");
            }
        };

        fn search_in(path: &PathBuf, needle: &str, results: &mut Vec<SearchResult>) {
            // Cap reached!
            if results.len() < 500 {
                // Skip hidden files
                if path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .starts_with(".")
                {
                    return;
                }

                if let Ok(file) = fs::File::open(path)
                    && let Ok(md) = file.metadata()
                    // Maximum 1 MB
                    && md.len() < 1 << 20
                    && let Ok(s) = fs::read_to_string(path)
                {
                    for (line_idx, line_text) in
                        s.lines().enumerate().filter(|(_, l)| l.contains(needle))
                    {
                        results.push(SearchResult {
                            path: path.clone(),
                            line_idx,
                            line_text: line_text.trim().to_string(),
                        });
                    }
                } else if let Ok(entries) = fs::read_dir(path) {
                    // Special case, ignore Rust target dir to prevent searching too many places
                    {
                        let mut path = path.clone();
                        path.push("CACHEDIR.TAG");
                        if path.exists() {
                            return;
                        }
                    }

                    for entry in entries {
                        let Ok(entry) = entry else { continue };
                        search_in(&entry.path(), needle, results);
                    }
                }
            }
        }

        let mut results = Vec::new();
        search_in(&path, &needle, &mut results);

        let mut buffer = Buffer::default();
        let cursor_id = buffer.start_session();

        Self {
            options: Options::new(results),
            path,
            needle,
            cursor_id,
            buffer,
            input: Input::filter(),
            preview: None,
        }
    }

    pub fn requested_height(&self) -> usize {
        !0
        // self.options.requested_height() + 3
    }

    fn update_completions(&mut self) {
        let filter = self.buffer.text.to_string().to_lowercase();
        self.options.apply_scoring(|e| {
            let name = format!("{}", e.path.display()).to_lowercase();
            if name == filter {
                Some((0, name.chars().count()))
            } else if name.starts_with(&filter) {
                Some((1, name.chars().count()))
            } else if name.contains(&filter) {
                Some((2, name.chars().count()))
            } else {
                None
            }
        });
    }
}

impl Element<()> for Searcher {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp<()>, Event> {
        let filter_str = self.buffer.text.to_string();
        let res = match event.to_action(|e| e.to_cancel().or_else(|| e.to_char().map(Action::Char)))
        {
            Some(Action::Cancel) => Ok(Resp::end(None)),
            _ => match self.options.handle(state, event).map(Resp::into_ended) {
                // Selecting a directory enters the directory
                Ok(Some(result)) => Ok(Resp::end(Some(Event::Action(Action::OpenFile(
                    result.path,
                    result.line_idx,
                ))))),
                Ok(None) => Ok(Resp::handled(None)),
                Err(event) => {
                    let res = match self
                        .input
                        .handle(
                            &mut state.clipboard,
                            &mut self.buffer,
                            self.cursor_id,
                            event,
                        )
                        .map(Resp::into_can_end)
                    {
                        Ok(x) => Ok(x),
                        Err(event) => {
                            if let Some((buffer, cursor_id, input, _)) = &mut self.preview {
                                input
                                    .handle(&mut state.clipboard, buffer, *cursor_id, event)
                                    .map(Resp::into_can_end)
                            } else {
                                Err(event)
                            }
                        }
                    };
                    res
                }
            },
        };

        if self.buffer.text.to_string() != filter_str {
            self.update_completions();
        }
        res
    }
}

#[derive(Clone, PartialEq)]
pub struct SearchResult {
    pub path: PathBuf,
    pub line_idx: usize,
    pub line_text: String,
}

impl Visual for SearchResult {
    fn render(&mut self, state: &State, frame: &mut Rect) {
        let name = match self.path.file_name().and_then(|n| n.to_str()) {
            Some(name) => format!("{name}"),
            None => format!("Unknown"),
        };
        frame
            .with_fg(state.theme.option_file)
            .text([0, 0], &format!("{name}:{}", self.line_idx + 1));
        frame.with_fg(state.theme.margin_line_num).with(|f| {
            f.text(
                [f.size()[0] as isize / 3, 0],
                &format!("{}", self.line_text),
            );
        });
    }
}

impl Visual for Searcher {
    fn render(&mut self, state: &State, frame: &mut Rect) {
        let path_input_sz = 3;
        let remaining_sz = frame.size()[1].saturating_sub(path_input_sz);
        let (preview_sz, options_sz) = if remaining_sz > 12 {
            let preview_sz = remaining_sz / 2;
            (preview_sz, remaining_sz - preview_sz)
        } else {
            (0, remaining_sz)
        };

        self.preview = self.options.selected().and_then(|result| {
            self.preview
                .take()
                .filter(|(_, _, _, r)| r == result)
                .or_else(|| {
                    let mut buffer = Buffer::open(result.path.clone()).ok()?;
                    let cursor_id = buffer.start_session();
                    let mut input = Input::default();
                    buffer.goto_cursor(cursor_id, [0, result.line_idx as isize], true);
                    input.focus([0, result.line_idx as isize - preview_sz as isize / 2]);
                    Some((buffer, cursor_id, input, result.clone()))
                })
        });

        if let Some((buffer, cursor_id, input, result)) = &mut self.preview {
            frame.rect([0, 0], [frame.size()[0], preview_sz]).with(|f| {
                input.render(state, buffer.name().as_deref(), buffer, *cursor_id, None, f)
            });
        }

        frame
            .rect([0, preview_sz], [frame.size()[0], options_sz])
            .with(|f| self.options.render(state, f));
        frame
            .rect(
                [0, preview_sz + options_sz],
                [frame.size()[0], path_input_sz],
            )
            .with(|f| {
                let title = format!(
                    "{} results for '{}' in {}/",
                    if self.options.ranking.is_empty() {
                        format!("No")
                    } else {
                        format!(
                            "{} of {}",
                            self.options.selected + 1,
                            self.options.ranking.len()
                        )
                    },
                    self.needle,
                    self.path.display()
                );
                self.input
                    .render(state, Some(&title), &self.buffer, self.cursor_id, None, f)
            });
    }
}
