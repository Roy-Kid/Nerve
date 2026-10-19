//! Generate website examples through the authoritative projection.
use nerve_hub::projection::{project_json, summarize};
use serde_json::{Value, json};
fn main() {
    let inputs = json!([
    {"id":"deploy","name":"deploy edge","alias":"edge","outcome":"failure"},
    {"id":"codex","name":"Codex · refactor","alias":"gpu-box","attention":{"level":"required","reason":"approval"}},
    {"id":"build","name":"xcodebuild Nerve","alias":"studio","current":{"type":"completed"}},
    {"id":"claude","name":"Claude Code","alias":"studio","current":{"type":"thinking"}},
    {"id":"stream","name":"build log","alias":"ci","current":{"type":"monitor"}},
    {"id":"watch","name":"Local watcher","alias":"ci","current":{"type":"idle"}}
    ]);
    let jobs: Vec<Value> = inputs
        .as_array()
        .unwrap()
        .iter()
        .cloned()
        .map(|mut j| {
            j["machineId"] = json!("website-machine");
            j["producer"] = json!({"id":"demo"});
            project_json(j)
        })
        .collect();
    let jobs = json!(jobs);
    let order: Vec<_> = jobs
        .as_array()
        .unwrap()
        .iter()
        .map(|j| j["id"].clone())
        .collect();
    println!(
        "{}",
        json!({"protocolVersion":2,"hubInstanceId":"website-hub","machineId":"website-machine","revision":1,"summary":summarize(&jobs),"order":order,"jobs":jobs,"departed":[],"notifications":[],"machines":[]})
    );
}
