#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;

use app::MyApp;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("get_iplayer_rs GUI"),
        ..Default::default()
    };

    eframe::run_native(
        "get_iplayer_rs_gui",
        options,
        Box::new(|cc| Ok(Box::new(MyApp::new(cc)))),
    )
}
