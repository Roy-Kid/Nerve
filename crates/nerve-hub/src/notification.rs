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
            for (kind, rank) in [
                ("ask", ask),
                ("failure", if failure { 3 } else { 0 }),
                ("unresponsive", if unresponsive { 3 } else { 0 }),
                ("longSuccess", if success { 3 } else { 0 }),
            ] {
                let key = (id.to_owned(), kind.to_owned());
                next.insert(key.clone(), rank);
                let fired = (id.to_owned(), kind.to_owned(), rank);
                if rank < 2
                    || self.previous.get(&key).copied().unwrap_or(0) >= rank
                    || self.fired.contains_key(&fired)
                {
                    continue;
                }
                let targets = roster.targets(job, rank, kind);
                if !targets.is_empty() {
                    self.fired.insert(fired, now);
                }
                for (target, connection, sound) in targets {
                    self.sequence += 1;
                    let name = job["name"].as_str().unwrap_or("Task");
                    let title = match kind {
                        "failure" => format!("Failed: {name}"),
                        "unresponsive" => format!("Unresponsive: {name}"),
                        "longSuccess" => format!("Completed: {name}"),
                        _ => job["attention"]["title"]
                            .as_str()
                            .unwrap_or("Return to your agent")
                            .to_owned(),
                    };
                    events.push(json!({"id":format!("n-{}-{}",std::process::id(),self.sequence),"target":target,"connectionId":connection,"jobId":id,"kind":kind,"title":title,
      "body":job["attention"]["summary"].as_str().or(job["current"]["summary"].as_str()).unwrap_or(""),"level":if kind=="ask" {job["attention"]["level"].clone()}else{json!("required")},"sound":sound}));
                }
            }
        }
        self.previous = next;
        events
    }
}
