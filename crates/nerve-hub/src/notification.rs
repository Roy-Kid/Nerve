//! Transition detection, channels, routing and deduplication belong to the hub.
use crate::lifecycle::SurfaceRoster;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::time::{Duration, Instant};
#[derive(Default)]
pub struct Notifications {
    previous: HashMap<(String, String), u64>,
    fired: HashMap<(String, String, u64), Instant>,
    sequence: u64,
}
impl Notifications {
    pub fn evaluate(&mut self, jobs: &Value, roster: &SurfaceRoster) -> Vec<Value> {
        self.evaluate_frame(jobs, &json!([]), roster)
    }
    pub fn evaluate_frame(
        &mut self,
        jobs: &Value,
        departed: &Value,
        roster: &SurfaceRoster,
    ) -> Vec<Value> {
        self.evaluate_frame_at(jobs, departed, roster, Instant::now())
    }
    pub fn evaluate_frame_at(
        &mut self,
        jobs: &Value,
        departed: &Value,
        roster: &SurfaceRoster,
        now: Instant,
    ) -> Vec<Value> {
        self.fired
            .retain(|_, t| now.duration_since(*t) < Duration::from_secs(120));
        let mut next = HashMap::new();
        let mut events = Vec::new();
        for job in jobs
            .as_array()
            .into_iter()
            .flatten()
            .chain(departed.as_array().into_iter().flatten())
        {
            let Some(id) = job["id"].as_str() else {
                continue;
            };
            let ask = if job["display"]["ask"] == true && job["lifecycle"] != "ended" {
                match job["attention"]["level"].as_str() {
                    Some("suggested") => 2,
                    Some("required") => 3,
                    Some("urgent") => 4,
                    _ => 0,
                }
            } else {
                0
            };
            let failure = job["outcome"] == "failure" || job["attention"]["reason"] == "failure";
            let unresponsive = job["health"] == "unresponsive";
            let success = job["lifecycle"] == "ended" && job["outcome"] == "success";
            let turn_complete =
                job["lifecycle"] == "active" && job["current"]["type"] == "completed";
            for (kind, rank) in [
                ("ask", ask),
                ("failure", if failure { 3 } else { 0 }),
                ("unresponsive", if unresponsive { 3 } else { 0 }),
                ("longSuccess", if success { 3 } else { 0 }),
                ("turnComplete", if turn_complete { 3 } else { 0 }),
            ] {
                let key = (id.to_owned(), kind.to_owned());
                // Every turn ends the same way, so its start tells two apart:
                // a quick next turn is not the one already announced.
                let occurrence = if kind == "turnComplete" {
                    format!(
                        "{kind}@{}",
                        job["current"]["startedAt"].as_str().unwrap_or("")
                    )
                } else {
                    kind.to_owned()
                };
                let fired = (id.to_owned(), occurrence, rank);
                let previous = self.previous.get(&key).copied().unwrap_or(0);
                if rank < 2 || previous >= rank || self.fired.contains_key(&fired) {
                    next.insert(key, rank);
                    continue;
                }
                let targets = roster.targets(job, rank, kind);
                if targets.is_empty() {
                    // Nobody could show it. Keep the old rank so the next frame
                    // still sees a rise once a surface is eligible.
                    if let Some(old) = self.previous.get(&key) {
                        next.insert(key.clone(), *old);
                    }
                    continue;
                }
                self.fired.insert(fired, now);
                next.insert(key, rank);
                for (target, connection, sound) in targets {
                    self.sequence += 1;
                    let name = job["name"].as_str().unwrap_or("Task");
                    let title = match kind {
                        "failure" => format!("Failed: {name}"),
                        "unresponsive" => format!("Unresponsive: {name}"),
                        "longSuccess" => format!("Completed: {name}"),
                        "turnComplete" => format!("Turn complete: {name}"),
                        _ => job["attention"]["title"]
                            .as_str()
                            .unwrap_or("Return to your agent")
                            .to_owned(),
                    };
                    events.push(json!({"id":format!("n-{}-{}",std::process::id(),self.sequence),"target":target,"connectionId":connection,"jobId":id,"kind":kind,"title":title,
      // The turn's own summary is "Turn complete", already the title.
      "body":if kind=="turnComplete" {""} else {job["attention"]["summary"].as_str().or(job["current"]["summary"].as_str()).unwrap_or("")},"level":if kind=="ask" {job["attention"]["level"].clone()}else{json!("required")},"sound":sound}));
                }
            }
        }
        self.previous = next;
        events
    }
}
