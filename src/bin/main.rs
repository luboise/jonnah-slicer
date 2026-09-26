#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use clap::Parser;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Parser, Debug)]
struct Args {
    audio_file: Option<std::path::PathBuf>,
}

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    let logging_mutex = jonnah_slicer::logging::init().map_err(|e| {
        eframe::Error::AppCreation(format!("failed to initialise logger: {e}").into())
    })?;

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_min_inner_size([300.0, 220.0])
            .with_icon(
                // NOTE: Adding an icon is optional
                eframe::icon_data::from_png_bytes(
                    &include_bytes!("../../assets/favicon-512x512.png")[..],
                )
                .expect("Failed to load icon"),
            )
            .with_drag_and_drop(true),

        ..Default::default()
    };

    eframe::run_native(
        "jonnah-slicer",
        native_options,
        Box::new(|cc| {
            Ok(Box::new(jonnah_slicer::JonnahSlicer::new(
                cc,
                logging_mutex,
            )))
        }),
    )
}
