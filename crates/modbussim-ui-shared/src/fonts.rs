//! Install a system CJK font into egui so Chinese / Japanese / Korean glyphs
//! render instead of tofu squares. egui's built-in default fonts cover Latin +
//! common symbols but no CJK.

use egui::{FontData, FontDefinitions, FontFamily};

/// Look up a CJK-capable system font and register it as a fallback for both
/// Proportional and Monospace families. Silently no-ops if nothing is found —
/// UI stays usable (just CJK shows as tofu).
pub fn install_cjk_fonts(ctx: &egui::Context) {
    let Some((name, bytes)) = load_first_available_cjk_font() else {
        log::warn!(
            "No system CJK font found; 中文 will render as tofu. \
             Consider shipping a bundled subset font."
        );
        return;
    };

    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert(name.to_string(), FontData::from_owned(bytes));

    // Append as the *last* entry so Latin glyphs still come from the default
    // fonts (which are tighter) and CJK falls through to our system font.
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

    ctx.set_fonts(fonts);
    log::info!("Installed CJK font: {}", name);
}

fn load_first_available_cjk_font() -> Option<(&'static str, Vec<u8>)> {
    const CANDIDATES: &[(&str, &str)] = &[
        // macOS
        ("PingFang", "/System/Library/Fonts/PingFang.ttc"),
        ("STHeiti", "/System/Library/Fonts/STHeiti Medium.ttc"),
        ("Hiragino", "/System/Library/Fonts/Hiragino Sans GB.ttc"),
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
