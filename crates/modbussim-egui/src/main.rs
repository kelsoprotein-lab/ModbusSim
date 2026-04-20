mod app;

use std::sync::Arc;

use eframe::egui;

/// Parse `--auto-tcp host:port` for dev smoke-tests. Returns (host, port) if given.
fn parse_auto_tcp() -> Option<(String, u16)> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--auto-tcp" {
            let spec = args.next()?;
            let (h, p) = spec.rsplit_once(':')?;
            let port: u16 = p.parse().ok()?;
            return Some((h.to_string(), port));
        }
    }
    None
}

/// `--auto-counter <addr>` for dev smoke-tests: seed a counter data source at addr.
fn parse_auto_counter() -> Option<u16> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--auto-counter" {
            return args.next()?.parse().ok();
        }
    }
    None
}

fn main() -> eframe::Result<()> {
    env_logger::init();

    let rt = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime"),
    );

    let auto_tcp = parse_auto_tcp();
    let auto_counter_addr = parse_auto_counter();

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
        Box::new(move |cc| {
            modbussim_ui_shared::fonts::install_cjk_fonts(&cc.egui_ctx);
            let flavor = cc
                .storage
                .and_then(|s| {
                    eframe::get_value::<modbussim_ui_shared::theme::Flavor>(s, "flavor_v3")
                })
                .unwrap_or_default();
            modbussim_ui_shared::theme::apply(&cc.egui_ctx, flavor);
            let mut app = app::SlaveApp::new(rt.clone(), flavor);
            if let Some((host, port)) = auto_tcp.clone() {
                app.auto_start_tcp(host, port);
                if let Some(addr) = auto_counter_addr {
                    app.auto_add_counter(addr);
                }
            }
            Ok(Box::new(app))
        }),
    )
}
