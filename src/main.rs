#![feature(bind_by_move_pattern_guards, euclidean_division, associated_type_defaults)]

mod config;
mod display;
mod input;
mod event;
mod draw;
mod ui;
mod buffer;
mod state;

use std::panic;
use backtrace::Backtrace;
use clap::{App, Arg};
use crate::{
    config::Config,
    display::Display,
    event::{Dir, Event},
    draw::Canvas,
    ui::MainUi,
    buffer::{Buffer, BufferMut, Line, SharedBufferRef},
    state::State,
};

const LOG_FILENAME: &str = concat!(env!("CARGO_PKG_NAME"), ".log");

fn setup() -> Config {
    // Set up panic hook
    panic::set_hook(Box::new(move |info| {
        log::error!("{}", info);
        log::error!("{:?}", Backtrace::new());
    }));

    // Enable logging
    simple_logging::log_to_file(LOG_FILENAME, log::LevelFilter::Info)
        .expect("Failed to enable logging");

    // Load config
    Config::load().unwrap_or_else(|err| {
        log::warn!("Failed to load config: {:?}", err);
        Config::default()
    })
}

fn main() {
    let config = setup();

    let matches = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(Arg::with_name("PATH")
            .help("Specify a file to edit")
            .multiple(true))
        .get_matches();

    let mut display = Display::new();
    let mut ui = MainUi::default();

    match matches.values_of("PATH") {
        Some(paths) => {
            let state = State::from_paths(paths.map(|path| path.to_string().into())).0;
            ui = ui.with_state(state);
        },
        None => {},
    }

    let event_rx = input::begin_reading();
    loop {
        ui.render(&mut display);
        display.render();

        match event_rx.recv().unwrap() {
            Event::Quit => {
                log::info!("Quitting...");
                break;
            },
            event => ui.handle(event),
        }
    }
}
