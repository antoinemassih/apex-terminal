//! apex-playground — standalone Storybook-like component showcase.
//!
//! Run with:
//!   cargo run --bin apex-playground
//!   cargo run --bin apex-playground --release
//!
//! No Tauri, no chart renderer, no trading data. Pure eframe + ui_kit.

// Pull in the library crate so we can use _scaffold_lib::ui_kit.
use _scaffold_lib as lib;

mod playground;

fn main() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Apex Playground — Widget Showcase")
            .with_inner_size([1280.0, 900.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "apex-playground",
        options,
        Box::new(|_cc| Ok(Box::new(playground::Playground::new()))),
    )
    .expect("eframe failed to start");

    // suppress unused-import warning — the lib crate is used via its types
    let _ = std::marker::PhantomData::<lib::AppError>;
}
