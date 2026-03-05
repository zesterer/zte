use super::*;
use crate::state::{Buffer, CursorId};
use std::{
    fs,
    io::Read as _,
    ops::Range,
    path::Path,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

pub struct Searcher {
    options: Options<SearchResult>,
    path: PathBuf,
    search_path: PathBuf,
    // Filter
    buffer: Buffer,
    cursor_id: CursorId,
    input: Input,
    preview: Option<(Buffer, CursorId, Input, SearchLoc)>,
}

impl Searcher {
    pub fn new(path: PathBuf, needle: Option<&regex::CompiledPattern>) -> Self {
        let search_path = util::workspace_dir(path.clone());
        let search_path = search_path.canonicalize().unwrap_or_else(|_| search_path);

        fn search_in(
            search_path: &Path,
            path: &Path,
            needle: Option<&regex::CompiledPattern>,
            results: &mut Vec<SearchResult>,
            // Extra paths that need searching
            to_search: &mut Vec<PathBuf>,
            // Can be anything, will be cleared anyway: only exists to reuse the allocation!
            buf: &mut String,
        ) {
            // Skip hidden files
            if path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .starts_with(".")
            {
                return;
            }

            // Avoid infinitely recursing through links
            if path.as_os_str().len() > 512 {
                return;
            }

            let Ok(meta) = fs::symlink_metadata(path) else {
                return;
            };

            // Maximum 1 MB, any larger and we don't include it in results when searching for a text pattern
            if needle.is_some() && meta.len() >= 1 << 20 {
                return;
            }

            if meta.is_symlink()
                && let Ok(link) = path.canonicalize()
                && link.starts_with(search_path)
            {
                // Skip links that point back into the search path: we'll be visiting them anyway!
                return;
            }

            if fs::File::open(path)
                .and_then(|mut f| {
                    buf.clear();
                    f.read_to_string(buf)
                })
                .is_ok()
            {
                let rdir = format!(
                    "./{}",
                    path.parent()
                        .and_then(|p| p.strip_prefix(search_path).ok()?.to_str())
                        .unwrap_or("unknown")
                );
                if let Some(needle) = needle {
                    let mut file_matches = 0;
                    for matching in needle.find_nonoverlapping_matches(&buf) {
                        // Find the line_idx, line_pos, and line_text of the match
                        let (mut line_idx, mut line_pos, mut line_text) = (0, 0, String::new());
                        for line in buf.split_inclusive('\n') {
                            if line_pos + line.len() > matching.start {
                                line_text = buf[line_pos..][..line.len()].to_string();
                                break;
                            } else {
                                line_idx += 1;
                                line_pos += line.len();
                            }
                        }

                        let make_preview = {
                            let path = path.to_path_buf();
                            move || {
                                // Construct a single-line preview of the result
                                let mut line_buffer = Buffer::file(false, &line_text, path);
                                let cursor_id = line_buffer.start_session();
                                let mut line_input = Input::search_result(line_idx);

                                // Focus the input properly
                                line_input.focus(
                                    line_buffer
                                        .text
                                        .to_coord(line_text.len() - line_text.trim_start().len()),
                                );
                                let line_matching = matching.start.saturating_sub(line_pos)
                                    ..matching.end.saturating_sub(line_pos);
                                line_buffer.select_cursor(cursor_id, line_matching);
                                (line_input, cursor_id, line_buffer)
                            }
                        };

                        results.push(SearchResult {
                            loc: SearchLoc {
                                path: path.to_path_buf(),
                                span: Some((line_idx, matching)),
                            },
                            rdir: rdir.clone(),
                            line: Some(Err(Box::new(make_preview))),
                        });
                        file_matches += 1;
                        if file_matches >= 150 {
                            break;
                        }
                    }
                } else {
                    results.push(SearchResult {
                        loc: SearchLoc {
                            path: path.to_path_buf(),
                            span: None,
                        },
                        rdir,
                        line: None,
                    });
                }
            } else if let Ok(entries) = fs::read_dir(path) {
                // Special case, ignore Rust target dir to prevent searching too many places
                if path.ends_with("target") {
                    let mut path = path.to_path_buf();
                    path.push("CACHEDIR.TAG");
                    if path.exists() {
                        return;
                    }
                }

                for entry in entries {
                    let Ok(entry) = entry else { continue };
                    to_search.push(entry.path());
                }
            }
        }

        let results = Mutex::new(Vec::new());
        let to_search = Mutex::new(vec![search_path.clone()]);
        let par = thread::available_parallelism().map_or(1, |p| p.get());
        let waiting = AtomicUsize::new(0);
        thread::scope(|s| {
            for _ in 0..par {
                s.spawn(|| {
                    let mut buf = String::new();
                    loop {
                        // Search the next path
                        if let Some(next) = { to_search.lock().unwrap().pop() } {
                            // Stop looking for results if we hit our quota
                            if { results.lock().unwrap().len() } >= 10_000 {
                                break;
                            } else {
                                let mut results_new = Vec::new();
                                let mut to_search_new = Vec::new();
                                search_in(
                                    &search_path,
                                    &next,
                                    needle,
                                    &mut results_new,
                                    &mut to_search_new,
                                    &mut buf,
                                );
                                // Only add the results of the search to the working data if we found anything
                                if !results_new.is_empty() {
                                    results.lock().unwrap().append(&mut results_new)
                                }
                                if !to_search_new.is_empty() {
                                    to_search.lock().unwrap().append(&mut to_search_new)
                                }
                            }
                        } else {
                            // Wait until every thread is waiting, which would imply that there's no more work to be done
                            if waiting.fetch_add(1, Ordering::Relaxed) + 1 >= par {
                                break;
                            } else {
                                // Wait for a while in the hope that more work will become available
                                thread::sleep(Duration::from_micros(10));
                                waiting.fetch_sub(1, Ordering::Relaxed);
                            }
                        }
                    }
                });
            }
        });
        let results = results.into_inner().unwrap();

        let mut buffer = Buffer::default();
        let cursor_id = buffer.start_session();

        let mut this = Self {
            options: Options::new(results),
            path,
            search_path,
            cursor_id,
            buffer,
            input: Input::filter(),
            preview: None,
        };
        this.update_completions();
        this
    }

    pub fn requested_height(&self) -> usize {
        !0
        // self.options.requested_height() + 3
    }

    fn update_completions(&mut self) {
        let filter = self.buffer.text.to_string().to_lowercase();
        self.options.apply_scoring(|e| {
            let name = e
                .loc
                .path
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("<unknown>")
                .to_lowercase();
            let parent = e
                .loc
                .path
                .parent()
                .and_then(|f| Some(f.to_str()?.to_lowercase()));
            let unshared_components = e
                .loc
                .path
                .ancestors()
                .map(|a| if self.path.starts_with(a) { -1 } else { 1 })
                .sum::<i32>();
            if name == filter {
                Some((0, unshared_components))
            } else if name.starts_with(&filter) {
                Some((1, unshared_components))
            } else if name.contains(&filter) {
                Some((2, unshared_components))
            } else if let Some(parent) = parent
                && parent.contains(&filter)
            {
                Some((3, unshared_components))
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
                    result.loc.path,
                    result.loc.span.clone().map(|(_, span)| span),
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
struct SearchLoc {
    path: PathBuf,
    // (line_idx, span)
    span: Option<(usize, Range<usize>)>,
}

struct SearchResult {
    loc: SearchLoc,
    rdir: String,
    line: Option<
        Result<
            (Input, CursorId, Buffer),
            Box<dyn FnOnce() -> (Input, CursorId, Buffer) + Send + Sync>,
        >,
    >,
}

impl Visual for SearchResult {
    fn render(&mut self, state: &mut State, frame: &mut Rect) {
        let name = match self.loc.path.file_name().and_then(|n| n.to_str()) {
            Some(name) => format!("{name}"),
            None => format!("Unknown"),
        };
        let col_a = (frame.size()[0] / 5).max(20);
        let col_b = frame.size()[0] / 3;
        // Filename
        frame
            .rect([0, 0], [col_a, !0])
            .with_theme(state.theme.option_file)
            .text([0, 0], &name);
        // Path
        frame
            .rect([col_a, 0], [col_b, !0])
            .with_theme(state.theme.option_dir)
            .text([0, 0], &self.rdir);
        // Resolve the search result preview
        self.line = match self.line.take() {
            Some(Err(f)) => Some(Ok(f())),
            l => l,
        };
        if let Some(line) = &mut self.line
            && let Ok((input, cursor_id, buffer)) = line
        {
            input.render(
                &state.theme,
                None,
                buffer,
                *cursor_id,
                None,
                &mut frame.rect([col_a + col_b, 0], [!0, !0]),
            );
        }
    }
}

impl Visual for Searcher {
    fn render(&mut self, state: &mut State, frame: &mut Rect) {
        let path_input_sz = 3;
        let remaining_sz = frame.size()[1].saturating_sub(path_input_sz);
        let (preview_sz, options_sz) = if remaining_sz > 12 {
            let options_sz = self.options.requested_height().min(remaining_sz / 2);
            (remaining_sz - options_sz, options_sz)
        } else {
            (0, remaining_sz)
        };

        self.preview = self.options.selected().and_then(|result| {
            self.preview
                .take()
                .filter(|(_, _, _, loc)| loc == &result.loc)
                .or_else(|| {
                    let mut buffer = Buffer::open(result.loc.path.clone()).ok()?;
                    let cursor_id = buffer.start_session();
                    let mut input = Input::default();
                    if let Some((line_idx, range)) = result.loc.span.clone() {
                        buffer.goto_cursor(cursor_id, [0, line_idx as isize], true);
                        input.refocus(&mut buffer, cursor_id);
                        buffer.select_cursor(cursor_id, range);
                    }
                    Some((buffer, cursor_id, input, result.loc.clone()))
                })
        });

        if let Some((buffer, cursor_id, input, _)) = &mut self.preview {
            frame.rect([0, 0], [frame.size()[0], preview_sz]).with(|f| {
                input.render(
                    &state.theme,
                    buffer.name().as_deref(),
                    buffer,
                    *cursor_id,
                    None,
                    f,
                )
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
                let num_results = if self.options.ranking.is_empty() {
                    format!("No")
                } else {
                    format!(
                        "{} of {}",
                        self.options.selected + 1,
                        self.options.ranking.len()
                    )
                };
                let title = format!("{} results in {}/", num_results, self.search_path.display());
                self.input.render(
                    &state.theme,
                    Some(&title),
                    &mut self.buffer,
                    self.cursor_id,
                    None,
                    f,
                )
            });
    }
}
