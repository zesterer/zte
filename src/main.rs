mod action;
mod highlight;
mod lang;
mod state;
mod terminal;
mod theme;
mod ui;
mod util;

use crate::{
    action::{Action, Dir, Dist, Event, MouseAction},
    state::State,
    terminal::{Area, Attribute, Attributes, Color, Terminal, TerminalEvent},
    ui::{Element as _, Visual as _},
};
use clap::Parser;
use futures::{FutureExt, StreamExt};
use std::{io, path::PathBuf, sync::Arc, time::Duration};

#[derive(Parser, Debug)]
struct Args {
    paths: Vec<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("No such buffer")]
    NoSuchBuffer,
    #[error("File is not yet on disk")]
    FileNotOnDisk,
    #[error("Environment variable `SHELL` does not exist")]
    NoShellEnvVar,
    #[error("Shell process failed to spawn: {0}")]
    ShellProcessFailed(pty_process::Error),
}

fn main() -> Result<(), Error> {
    let args = Args::parse();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .enable_io()
        .build()?;
    let notify = Arc::new(tokio::sync::Notify::new());

    let mut state = State::new(&args, notify.clone());
    let mut ui = ui::Root::new(&mut state, &args);

    Terminal::with(move |term| {
        rt.block_on(async {
            let mut events = term.event_stream();
            let mut interval = tokio::time::interval(Duration::from_millis(250));
            let mut close_requested = false;

            while !close_requested && !ui.should_close() {
                let mut handle_event = |event| {
                    let event = match &event {
                        Event::Raw(ev) => {
                            // Resize events are special and need handling by the terminal
                            if let TerminalEvent::Resize(cols, rows) = &ev.0 {
                                state.needs_render = true;
                                term.set_size([*cols, *rows]);
                                Event::Tick // Actually a resize, but we don't consider resizing to be special
                            } else if let TerminalEvent::Paste(s) = &ev.0 {
                                let _ = state.clipboard.set_no_dirty(s.clone());
                                Event::Action(Action::Paste)
                            } else {
                                event
                            }
                        }
                        Event::Internal => {
                            // Usually caused by a change to a terminal pane, so trigger a render
                            state.needs_render = true;
                            event
                        }
                        Event::Tick => {
                            state.tick();
                            event
                        }
                        _ => event,
                    };

                    // Have the UI handle the event
                    match ui.handle(&mut state, event) {
                        Ok(r) if r.is_end() => close_requested = true,
                        Ok(_) => state.needs_render = true,
                        Err(Event::Action(Action::Bell)) => term.frame().ring_bell(),
                        // Unhandled event!
                        Err(_) => {}
                    }
                };

                // Wait for the next event
                handle_event(tokio::select! {
                    ev = events.next() => if let Some(Ok(ev)) = ev {
                        Event::from_raw(ev)
                    } else {
                        // Ummm...?
                        Event::Tick
                    },
                    _ = notify.notified() => Event::Internal,
                    _ = interval.tick() => Event::Tick,
                });

                // Now that we're awake, speculatively process any extra events that happen to be immediately
                // available (or very soon after - we can't control the latency of terminal processes!) to
                // avoid wasting renders
                let soon = tokio::time::Instant::now(); // + Duration::from_millis(5);
                while let Ok(Some(Ok(ev))) = tokio::time::timeout_at(soon, events.next()).await {
                    let _ = notify.notified().now_or_never(); // Clear any pending notifications - we're about to handle them!
                    handle_event(Event::from_raw(ev));
                }

                // If the clipboard has changed, tell the host terminal about it
                if let Some(content) = state.clipboard.get_local_clear_dirty() {
                    term.copy(content);
                }

                // Render the state to the screen
                if state.needs_render {
                    state.needs_render = false;
                    state.pre_render();
                    term.update(|fb| {
                        ui.render(&mut state, fb);
                    });
                }
            }

            Ok(())
        })
    })
}
