//! Install a system CJK font into egui so Chinese / Japanese / Korean glyphs
//! render instead of tofu squares, plus the Phosphor icon font so widgets can
//! use `icons::ARROW_DOWN` etc. egui's built-in default fonts cover Latin but
//! no CJK and no icon glyphs.

use egui::{FontData, FontDefinitions, FontFamily};

/// Look up a CJK-capable system font and register it as a fallback for both
/// Proportional and Monospace families. Silently no-ops if nothing is found —
/// UI stays usable (just CJK shows as tofu).
pub fn install_cjk_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    // CJK fallback.
    match load_first_available_cjk_font() {
        Some((name, bytes)) => {
            fonts
                .font_data
                .insert(name.to_string(), std::sync::Arc::new(FontData::from_owned(bytes)));
            fonts
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .push(name.to_string());
            fonts
                .families
                .entry(FontFamily::Monospace)
                .or_default()
                .push(name.to_string());
            log::info!("Installed CJK font: {}", name);
        }
        None => {
            log::warn!(
                "No system CJK font found; 中文 will render as tofu. \
                 Consider shipping a bundled subset font."
            );
        }
    }

    // Phosphor icons — TTF is bundled inside egui-phosphor. Registers as last
    // fallback so arbitrary `icons::*` chars resolve.
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

    ctx.set_fonts(fonts);
}

fn load_first_available_cjk_font() -> Option<(&'static str, Vec<u8>)> {
    const CANDIDATES: &[(&str, &str)] = &[
        // macOS — PingFang (if present on newer systems)
        ("PingFang-SC", "/System/Library/Fonts/PingFang.ttc"),
        ("PingFang-Supp", "/System/Library/Fonts/Supplemental/PingFang SC.ttf"),
        ("PingFang-App", "/Library/Fonts/PingFang.ttc"),
        // STHeiti Medium — bolder than Hiragino, renders with more body at 13px
        ("STHeiti", "/System/Library/Fonts/STHeiti Medium.ttc"),
        ("HeitiSC", "/System/Library/Fonts/Supplemental/Heiti SC.ttc"),
        ("Hiragino", "/System/Library/Fonts/Hiragino Sans GB.ttc"),
        ("HiraginoAlt", "/Library/Fonts/Hiragino Sans GB.ttc"),
        ("STHeitiLight", "/System/Library/Fonts/STHeiti Light.ttc"),
        ("ArialUnicode", "/Library/Fonts/Arial Unicode.ttf"),
        // Windows
        ("MSYH", "C:\\Windows\\Fonts\\msyh.ttc"),
        ("MSYH-TTF", "C:\\Windows\\Fonts\\msyh.ttf"),
        ("SimSun", "C:\\Windows\\Fonts\\simsun.ttc"),
        // Linux (Debian/Ubuntu Noto CJK)
        (
            "NotoCJK",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        ),
        (
            "NotoCJK-OTF",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        ),
        // Linux (Fedora / Arch common)
        (
            "NotoCJK-Arch",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        ),
        (
            "WQYZenHei",
            "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
        ),
    ];

    for (name, path) in CANDIDATES {
        match std::fs::read(path) {
            Ok(bytes) => return Some((name, bytes)),
            Err(_) => continue,
        }
    }
    None
}
