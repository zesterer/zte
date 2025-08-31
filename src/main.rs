mod action;
mod highlight;
mod state;
mod terminal;
mod theme;
mod ui;

use crate::{
    action::{Action, Dir, Event},
    state::State,
    terminal::{Color, Terminal, TerminalEvent},
    ui::{Element as _, Visual as _},
};
use clap::Parser;
use std::{io, path::PathBuf, time::Duration};

#[derive(Parser, Debug)]
struct Args {
    paths: Vec<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] io::Error),
}

fn main() -> Result<(), Error> {
    let args = Args::parse();

    let mut state = State::try_from(args)?;
    let open_buffers = state.buffers.keys().collect::<Vec<_>>();
    let mut ui = ui::Root::new(&mut state, &open_buffers);

    Terminal::with(move |term| {
        loop {
            // Render the state to the screen
            term.update(|fb| ui.render(&state, fb));

            // Wait for a while
            term.wait_at_least(Duration::from_millis(250));
            state.tick();

            while let Some(ev) = term.get_event() {
                // Resize events are special and need handling by the terminal
                if let TerminalEvent::Resize(cols, rows) = ev {
                    term.set_size([cols, rows]);
                }

                // Have the UI handle events
                if ui
                    .handle(&mut state, Event::from_raw(ev))
                    .map_or(false, |r| r.into_ended().is_some())
                {
                    return Ok(());
                }
            }
        }
    })
}
