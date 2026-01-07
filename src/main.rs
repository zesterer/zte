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
use std::{io, path::PathBuf, time::Duration};

#[derive(Parser, Debug)]
struct Args {
    paths: Vec<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("no such buffer")]
    NoSuchBuffer,
}

fn main() -> Result<(), Error> {
    let args = Args::parse();

    let mut state = State::new(&args);
    let mut ui = ui::Root::new(&mut state, &args);

    Terminal::with(move |term| {
        let mut needs_render = true;
        loop {
            while let Some(ev) = term.get_event() {
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

            state.tick(&mut needs_render);

            // Render the state to the screen
            if needs_render {
                term.update(|fb| ui.render(&state, fb));
            }

            // Wait for a while, or until an event occurs
            term.wait_at_least(Duration::from_millis(250));
        }
    })
}
