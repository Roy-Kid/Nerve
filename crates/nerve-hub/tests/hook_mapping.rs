//! Authoritative host event mapping fixtures. Plugins have no classifiers.
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use nerve_hub::hook::{Facets, map_event};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn fixtures_dir() -> PathBuf {
    repo_root().join("fixtures").join("hook-events")
}

/// Flatten hub `Facets` into the same nested shape `nerve.js` / `nerve.py` emit.
fn facets_to_value(f: &Facets) -> Value {
    json!({
        "lifecycle": f.lifecycle,
        "current": {
            "type": f.current_type,
            "name": f.current_name,
            "summary": f.current_summary,
        },
        "attention": {
            "level": f.attention_level,
            "reason": f.attention_reason,
            "title": f.attention_title,
            "summary": f.attention_summary,
        },
        "health": f.health,
        "outcome": f.outcome,
        "ended": f.ended,
        "end_reason": f.end_reason,
    })
}

/// Canonical facets for diffing: every key present, `startedAt` stripped.
fn normalize(raw: &Value) -> Value {
    if raw.is_null() {
        return Value::Null;
    }
    let cur = raw.get("current");
    let att = raw.get("attention");
    let pick =
        |v: Option<&Value>, k: &str| v.and_then(|o| o.get(k)).cloned().unwrap_or(Value::Null);
    json!({
        "lifecycle": raw.get("lifecycle").cloned().unwrap_or(Value::Null),
        "current": {
            "type": pick(cur, "type"),
            "name": pick(cur, "name"),
            "summary": pick(cur, "summary"),
        },
        "attention": {
            "level": pick(att, "level"),
            "reason": pick(att, "reason"),
            "title": pick(att, "title"),
            "summary": pick(att, "summary"),
        },
        "health": raw.get("health").cloned().unwrap_or(Value::Null),
        "outcome": raw.get("outcome").cloned().unwrap_or(Value::Null),
        "ended": json!(raw.get("ended").and_then(Value::as_bool).unwrap_or(false)),
        "end_reason": raw.get("end_reason").cloned().unwrap_or(Value::Null),
    })
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}

#[test]
fn hub_maps_every_host_event_fixture() {
    let dir = fixtures_dir();
    assert!(dir.is_dir(), "missing fixtures at {}", dir.display());

    let mut names: Vec<String> = fs::read_dir(&dir)
        .expect("read fixtures")
        .filter_map(|e| {
            let p = e.expect("dir entry").path();
            let fname = p.file_name()?.to_string_lossy().into_owned();
            if fname.ends_with(".json") && !fname.ends_with(".facets.json") {
                Some(fname.trim_end_matches(".json").to_owned())
            } else {
                None
            }
        })
        .collect();
    names.sort();
    assert!(!names.is_empty(), "no fixtures in {}", dir.display());

    let mut failures: Vec<String> = Vec::new();
    for name in &names {
        let input_path = dir.join(format!("{name}.json"));
        let expected_path = dir.join(format!("{name}.facets.json"));
        let input: Value = serde_json::from_str(&fs::read_to_string(&input_path).expect("input"))
            .expect("input json");
        let expected: Value =
            serde_json::from_str(&fs::read_to_string(&expected_path).expect("expected"))
                .expect("expected json");

        let hub = normalize(
            &map_event(&input)
                .map(|f| facets_to_value(&f))
                .unwrap_or(Value::Null),
        );
        let exp = normalize(&expected);

        if hub != exp {
            failures.push(format!(
                "{name}\n expected {}\n hub {}",
                pretty(&exp),
                pretty(&hub)
            ));
        }
    }

    if !failures.is_empty() {
        panic!(
            "{} of {} fixture(s) diverged:\n\n{}",
            failures.len(),
            names.len(),
            failures.join("\n\n")
        );
    }
}
