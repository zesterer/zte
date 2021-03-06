#![feature(associated_type_defaults, or_patterns)]

mod config;
mod display;
mod input;
mod event;
mod draw;
mod ui;
mod buffer;
mod state;

use std::{panic, env};
use backtrace::Backtrace;
use clap::{App, Arg};
use crate::{
    config::Config,
    display::{Display, Color},
    event::{Dir, Event},
    draw::Canvas,
    ui::{MainUi, Theme},
    buffer::{BufferId, BufferHandle, Line, Cursor, CursorId},
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
    if let Ok(fname) = env::var("ZTE_LOG") {
        simple_logging::log_to_file(
            if fname.len() == 0 {
                &fname
            } else {
                LOG_FILENAME
            },
            log::LevelFilter::Info,
        )
            .expect("Failed to enable logging");
    }

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

    let state = match matches.values_of("PATH") {
        Some(paths) => State::from_paths(paths.map(|path| path.to_string().into())).0,
        None => State::default(),
    };

    let mut ui = MainUi::new(Theme::default(), state);

    let event_rx = input::begin_reading();
    loop {
        ui.update(&mut display);
        ui.render(&mut display);
        display.render();

        if ui.handle(event_rx.recv().unwrap()) {
            log::info!("Quitting...");
            break;
        }
    }
}
