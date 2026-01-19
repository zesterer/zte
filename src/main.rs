mod action;
mod highlight;
mod lang;
mod state;
mod terminal;
mod theme;
mod ui;

use crate::{
    action::{Action, Dir, Dist, Event, MouseAction},
    state::State,
    terminal::{Area, Color, Terminal, TerminalEvent},
    ui::{Element as _, Visual as _},
};
use clap::Parser;
use futures::StreamExt;
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
            let mut needs_render = true;
            let mut events = term.event_stream();
            let mut interval = tokio::time::interval(Duration::from_millis(250));

            loop {
                let event = tokio::select! {
                    ev = events.next() => {
                        if let Some(Ok(ev)) = ev {
                            needs_render = true; // TODO: Don't always rerender?

                            // Resize events are special and need handling by the terminal
                            if let TerminalEvent::Resize(cols, rows) = &ev {
                                term.set_size([*cols, *rows]);
                                Event::Tick // Actually a resize, but we don't consider resizing to be special
                            } else if let TerminalEvent::Paste(s) = &ev {
                                let _ = state.clipboard.set_no_dirty(s.clone());
                                Event::Action(Action::Paste)
                            } else {
                                Event::from_raw(ev)
                            }
                        } else {
                            // Ummm...?
                            Event::Tick
                        }
                    },
                    _ = notify.notified() => Event::Tick,
                    _ = interval.tick() => Event::Tick,
                };

                // Have the UI handle the event
                match ui.handle(&mut state, event) {
                    Ok(r) if r.is_end() => return Ok(()),
                    Ok(_) => {}
                    Err(Event::Action(Action::Bell)) => term.ring_bell(),
                    // Unhandled event!
                    Err(_) => {}
                }

                state.tick(&mut needs_render);

                if let Some(content) = state.clipboard.get_local_clear_dirty() {
                    term.copy(content);
                }

                // Render the state to the screen
                if needs_render {
                    term.update(|fb| ui.render(&mut state, fb));
                }
            }
        })
    })
}
