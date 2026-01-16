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
                tokio::select! {
                    ev = events.next() => {
                        if let Some(Ok(ev)) = ev {
                            needs_render = true; // TODO: Don't always rerender?

                            // Resize events are special and need handling by the terminal
                            if let TerminalEvent::Resize(cols, rows) = ev {
                                term.set_size([cols, rows]);
                            }

                            // Have the UI handle events
                            match ui.handle(&mut state, Event::from_raw(ev)) {
                                Ok(r) if r.is_end() => return Ok(()),
                                Ok(_) => {}
                                Err(Event::Bell) => term.ring_bell(),
                                // Unhandled event!
                                Err(_) => {}
                            }
                        }
                    },
                    _ = notify.notified() => {},
                    _ = interval.tick() => {},
                }

                state.tick(&mut needs_render);

                // Render the state to the screen
                if needs_render {
                    term.update(|fb| ui.render(&mut state, fb));
                }
            }
        })
    })
}
