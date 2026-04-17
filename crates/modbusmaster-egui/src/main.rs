mod app;

use std::sync::Arc;

use eframe::egui;

fn main() -> eframe::Result<()> {
    env_logger::init();

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
            .with_title("ModbusMaster (egui)"),
        ..Default::default()
    };

    eframe::run_native(
        "ModbusMaster",
        native_options,
        Box::new(move |cc| {
            modbussim_ui_shared::fonts::install_cjk_fonts(&cc.egui_ctx);
            Ok(Box::new(app::MasterApp::new(rt.clone())))
        }),
    )
}
