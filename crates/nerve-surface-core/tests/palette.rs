//! The six product colours, and the rules that keep them six.

use std::path::PathBuf;

use nerve_surface_core::palette::{self, Rgb};
use nerve_surface_core::status::StatusClass;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn quantize(channel: f64) -> u8 {
    (channel * 255.0).round() as u8
}

fn rgb_call(text: &str) -> Option<(u8, u8, u8)> {
    let start = text.find("RGBColor(")?;
    let body = &text[start + "RGBColor(".len()..];
    let end = body.find(')')?;
    let mut red = None;
    let mut green = None;
    let mut blue = None;
    for part in body[..end].split(',') {
        let (key, value) = part.split_once(':')?;
        let number: f64 = value.trim().parse().ok()?;
        match key.trim() {
            "r" => red = Some(quantize(number)),
            "g" => green = Some(quantize(number)),
            "b" => blue = Some(quantize(number)),
            _ => {}
        }
    }
    Some((red?, green?, blue?))
}

#[test]
fn the_documented_hex_values_are_what_ship() {
    // These strings appear in `SettingsStore.swift`, in the tmux palette's doc
    // comment and on the site. If one of them changes, it changes here first.
    assert_eq!(palette::PROBLEM.hex(), "#FF3B30");
    assert_eq!(palette::ATTENTION.hex(), "#FF9F0A");
    assert_eq!(palette::RUNNING.hex(), "#0A84FF");
    assert_eq!(palette::MONITOR.hex(), "#BF5AF2");
    assert_eq!(palette::SUCCESS.hex(), "#30D158");
    assert_eq!(palette::INACTIVE.hex(), "#8E8E93");
}

#[test]
fn waiting_is_neutral_and_distinct_from_human_attention() {
    assert_eq!(
        palette::color_of(StatusClass::Waiting),
        palette::color_of(StatusClass::Inactive)
    );
}

#[test]
fn every_status_paints_one_of_six() {
    let all = [
        StatusClass::Problem,
        StatusClass::Attention,
        StatusClass::Waiting,
        StatusClass::Running,
        StatusClass::Monitor,
        StatusClass::Success,
        StatusClass::Inactive,
    ];
    let mut seen: Vec<Rgb> = all.iter().map(|c| palette::color_of(*c)).collect();
    seen.sort_by_key(|c| (c.r, c.g, c.b));
    seen.dedup();
    assert_eq!(seen.len(), 6, "seven statuses, six colours");
}

#[test]
fn blending_zero_changes_nothing_and_one_arrives() {
    assert_eq!(
        palette::PROBLEM.blend(palette::WHITE, 0.0),
        palette::PROBLEM
    );
    assert_eq!(palette::PROBLEM.blend(palette::WHITE, 1.0), palette::WHITE);
}

#[test]
fn a_lift_moves_towards_the_blend_target() {
    let lifted = palette::RUNNING.blend(palette::WHITE, 0.04);
    assert!(lifted.r > palette::RUNNING.r, "lift brightens");
    assert!(lifted.r - palette::RUNNING.r < 20, "and only slightly");
}

#[test]
fn a_fraction_outside_the_range_is_clamped() {
    assert_eq!(
        palette::PROBLEM.blend(palette::WHITE, -1.0),
        palette::PROBLEM
    );
    assert_eq!(palette::PROBLEM.blend(palette::WHITE, 9.0), palette::WHITE);
}

fn expect_hex(name: &str, hex: &str) {
    let color = match name {
        "problem" => palette::PROBLEM,
        "attention" => palette::ATTENTION,
        "waiting" | "inactive" => palette::INACTIVE,
        "running" => palette::RUNNING,
        "monitor" => palette::MONITOR,
        "success" => palette::SUCCESS,
        other => panic!("unknown status {other}"),
    };
    assert!(
        hex.eq_ignore_ascii_case(&color.hex()),
        "{name} {hex} is not {}",
        color.hex()
    );
}

/// Swift floats, the VS Code map, and the site tokens are the same six bytes.
#[test]
fn surfaces_copy_the_palette_bytes() {
    let root = repo_root();
    let swift = std::fs::read_to_string(root.join("Nerve/Nerve/Store/SettingsStore.swift"))
        .expect("SettingsStore");
    let monitor = rgb_call(
        swift
            .split("monitorDefault")
            .nth(1)
            .expect("monitorDefault"),
    )
    .expect("monitorDefault rgb");
    assert_eq!(
        monitor,
        (palette::MONITOR.r, palette::MONITOR.g, palette::MONITOR.b)
    );

    let default_at = swift
        .find("static let `default` = StatusColorMap(")
        .expect("default palette");
    let block = &swift[default_at..];
    let end = block.find("\n    )").expect("end of default palette");
    for line in block[..end].lines() {
        let Some((name, rest)) = line.trim().trim_end_matches(',').split_once(':') else {
            continue;
        };
        let name = name.trim();
        if !matches!(
            name,
            "problem" | "attention" | "waiting" | "running" | "monitor" | "success" | "inactive"
        ) {
            continue;
        }
        let bytes = if rest.contains("monitorDefault") {
            monitor
        } else {
            rgb_call(rest).unwrap_or_else(|| panic!("rgb for {name}"))
        };
        let color = match name {
            "problem" => palette::PROBLEM,
            "attention" => palette::ATTENTION,
            "waiting" | "inactive" => palette::INACTIVE,
            "running" => palette::RUNNING,
            "monitor" => palette::MONITOR,
            "success" => palette::SUCCESS,
            _ => unreachable!(),
        };
        assert_eq!(bytes, (color.r, color.g, color.b), "{name}");
    }

    let ts = std::fs::read_to_string(root.join("vsc-ext/src/model/job.ts")).expect("job.ts");
    let start = ts.find("STATUS_COLORS").expect("STATUS_COLORS");
    let body = &ts[start..ts[start..].find("};").map(|i| start + i).expect("end")];
    for line in body.lines() {
        let Some((name, rest)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if !matches!(
            name,
            "problem" | "attention" | "waiting" | "running" | "monitor" | "success" | "inactive"
        ) {
            continue;
        }
        let hex = rest
            .trim()
            .trim_matches(|c: char| c == '"' || c == ',' || c == ' ');
        expect_hex(name, hex);
    }

    let css = std::fs::read_to_string(root.join("index/src/styles/theme.css")).expect("theme.css");
    for line in css.lines() {
        let Some(rest) = line.trim().strip_prefix("--color-status-") else {
            continue;
        };
        let Some((name, value)) = rest.split_once(':') else {
            continue;
        };
        let hex = value.trim().trim_end_matches(';');
        expect_hex(name, hex);
    }
}
