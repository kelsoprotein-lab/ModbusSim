use std::sync::Arc;

use eframe::egui;

fn main() -> eframe::Result<()> {
    env_logger::init();

    // Tokio runtime lives for the lifetime of the app; UI tasks spawn onto it.
    let rt = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime"),
    );

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("ModbusSlave (egui)"),
        ..Default::default()
    };

    eframe::run_native(
        "ModbusSlave",
        native_options,
        Box::new(move |_cc| Ok(Box::new(SlaveApp::new(rt.clone())))),
    )
}

struct SlaveApp {
    _rt: Arc<tokio::runtime::Runtime>,
}

impl SlaveApp {
    fn new(rt: Arc<tokio::runtime::Runtime>) -> Self {
        Self { _rt: rt }
    }
}

impl eframe::App for SlaveApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("ModbusSlave — egui edition");
            ui.label("S0 skeleton: empty window is running.");
            ui.separator();
            ui.label("Next up: connection panel, register table, log panel.");
        });
    }
}
