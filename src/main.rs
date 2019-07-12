mod config;
mod display;
mod input;
mod event;
mod draw;
mod ui;

use std::panic;
use crate::{
    config::Config,
    display::Display,
    event::Event,
    draw::Canvas,
    ui::MainUi,
};

const LOG_FILENAME: &str = concat!(env!("CARGO_PKG_NAME"), ".log");

fn main() {
    let config = Config::load().unwrap_or_else(|err| {
        log::warn!("Failed to load config: {:?}", err);
        Config::default()
    });

    panic::set_hook(Box::new(move |info| {
        log::error!("{}", info);
    }));

	simple_logging::log_to_file(LOG_FILENAME, log::LevelFilter::Info)
        .expect("Failed to enable logging");

    let mut display = Display::new();
    let mut ui = MainUi::default();

    let event_rx = input::begin_reading(display.input());
    loop {
        ui.render(&mut display);
        display.render();

        match event_rx.recv().unwrap() {
            Event::Quit => {
                log::info!("Quitting...");
                break;
            },
        }
    }
}
