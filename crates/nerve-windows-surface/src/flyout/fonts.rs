//! Making the panel able to render what a job is actually called.
//!
//! egui's built-in font has no CJK glyphs, and this repo's own job names are
//! routinely Chinese. Bundling a CJK face would add megabytes and a licence
//! question for something every Windows install already has, so the system
//! faces are loaded instead and the built-in stays as the fallback.
//!
//! Every step is optional. A machine missing a font renders tofu, which is bad;
//! a surface that refused to start over it would be worse.

use std::path::PathBuf;

use egui::{FontData, FontDefinitions, FontFamily};

/// System faces to try, widest coverage first.
///
/// Yu Gothic and Malgun cover Japanese and Korean; Microsoft YaHei covers
/// Simplified Chinese and is present on every modern Windows.
#[cfg(windows)]
const CANDIDATES: [&str; 4] = ["msyh.ttc", "msyhl.ttc", "YuGothM.ttc", "malgun.ttf"];

/// macOS equivalents, so the panel can be looked at during development.
#[cfg(target_os = "macos")]
const CANDIDATES: [&str; 2] = ["PingFang.ttc", "Hiragino Sans GB.ttc"];

#[cfg(not(any(windows, target_os = "macos")))]
const CANDIDATES: [&str; 0] = [];

fn font_dirs() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("WINDIR")
            .map(|windir| vec![PathBuf::from(windir).join("Fonts")])
            .unwrap_or_default()
    }
    #[cfg(not(windows))]
    {
        vec![
            PathBuf::from("/System/Library/Fonts"),
            PathBuf::from("/Library/Fonts"),
        ]
    }
}

/// Add a system CJK face to `definitions`, if one can be found.
///
/// Returns whether anything was added, so a caller can say so in a log rather
/// than leave a user wondering why their job names are boxes.
pub fn install_cjk(definitions: &mut FontDefinitions) -> bool {
    // Use the Windows UI face for Latin text; keep egui's symbol fallback.
    #[cfg(windows)]
    for directory in font_dirs() {
        if let Ok(bytes) = std::fs::read(directory.join("segoeui.ttf")) {
            let name = "Segoe UI".to_string();
            definitions.font_data.insert(
                name.clone(),
                std::sync::Arc::new(FontData::from_owned(bytes)),
            );
            definitions
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .insert(0, name);
            break;
        }
    }
    let Some((name, bytes)) = load_first() else {
        return false;
    };
    definitions.font_data.insert(
        name.clone(),
        std::sync::Arc::new(FontData::from_owned(bytes)),
    );

    // Appended, not prepended: the built-in face is better shaped for Latin
    // and for egui's own icons, so it should keep those and defer only for
    // glyphs it does not have.
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        definitions
            .families
            .entry(family)
            .or_default()
            .push(name.clone());
    }
    true
}

fn load_first() -> Option<(String, Vec<u8>)> {
    for directory in font_dirs() {
        for candidate in CANDIDATES {
            let path = directory.join(candidate);
            if let Ok(bytes) = std::fs::read(&path) {
                return Some((candidate.to_string(), bytes));
            }
        }
    }
    None
}
