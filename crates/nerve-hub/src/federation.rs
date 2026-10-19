//! Hub-owned SSH connections and origin-scoped, non-transitive replication.
use crate::{sse::ChangeSignal, state::JobStore};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

fn config_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    home.join(".config/nerve")
}
pub fn machine_id() -> String {
    use std::io::Write;
    let directory = config_dir();
    let path = directory.join("machine-id");
    if let Ok(id) = std::fs::read_to_string(&path)
        && !id.trim().is_empty()
    {
        return id.trim().into();
    }
    let _ = std::fs::create_dir_all(&directory);
    let candidate = format!(
        "m-{}-{}",
        time::OffsetDateTime::now_utc().unix_timestamp_nanos(),
        std::process::id()
    );
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(mut file) => {
            let _ = writeln!(file, "{candidate}");
            candidate
        }
        Err(_) => {
            for _ in 0..20 {
                if let Ok(id) = std::fs::read_to_string(&path)
                    && !id.trim().is_empty()
                {
                    return id.trim().to_string();
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            candidate
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineConfig {
    pub id: String,
    pub host: String,
    #[serde(default)]
    pub auto_connect: bool,
    #[serde(default)]
    pub enabled: bool,
}
#[derive(Clone)]
pub struct Machines {
    store: Arc<Mutex<JobStore>>,
    changes: ChangeSignal,
    configs: Arc<Mutex<BTreeMap<String, MachineConfig>>>,
    active: Arc<Mutex<BTreeMap<String, Arc<AtomicBool>>>>,
}
impl Machines {
    pub fn new(store: Arc<Mutex<JobStore>>, changes: ChangeSignal) -> Self {
        let configs: Vec<MachineConfig> = std::fs::read(config_dir().join("machines.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let this = Self {
            store,
            changes,
            configs: Arc::new(Mutex::new(
                configs.into_iter().map(|c| (c.id.clone(), c)).collect(),
            )),
            active: Arc::default(),
        };
        for config in this.configs.lock().unwrap().values() {
            this.set_state(config, "disconnected", None);
        }
        this
    }
    pub fn start(&self) {
        if let Err(error) = self.discover() {
            tracing::warn!(%error,"SSH discovery failed");
        }
        let configs: Vec<_> = self
            .configs
            .lock()
            .unwrap()
            .values()
            .filter(|c| c.auto_connect && c.enabled)
            .cloned()
            .collect();
        for config in configs {
            let _ = self.connect(&config.id);
        }
    }
    pub fn discover(&self) -> Result<(), String> {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .ok_or("home directory missing")?;
        let path = home.join(".ssh/config");
        if let Ok(text) = std::fs::read_to_string(&path) {
            // Only remove a complete Nerve-owned block, never unrelated SSH directives.
            if let (Some(begin), Some(end)) = (
                text.find("# BEGIN NERVE MANAGED"),
                text.find("# END NERVE MANAGED"),
            ) && end > begin
            {
                let end = end + "# END NERVE MANAGED".len();
                let updated = format!("{}{}", &text[..begin], &text[end..]);
                let temporary = path.with_extension("nerve.tmp");
                std::fs::write(&temporary, updated).map_err(|e| e.to_string())?;
                if let Ok(metadata) = std::fs::metadata(&path) {
                    let _ = std::fs::set_permissions(&temporary, metadata.permissions());
                }
                std::fs::rename(temporary, &path).map_err(|e| e.to_string())?;
            }
        }
        let mut hosts = Vec::new();
        read_hosts(&path, &home, 0, &mut hosts);
        hosts.sort();
        hosts.dedup();
        for host in hosts {
            if self
                .configs
                .lock()
                .unwrap()
                .values()
                .any(|c| c.host == host)
            {
                continue;
            }
            let hex = format!(
                "{:032x}",
                time::OffsetDateTime::now_utc().unix_timestamp_nanos() as u128
            );
            let id = format!(
                "{}-{}-{}-{}-{}",
                &hex[..8],
                &hex[8..12],
                &hex[12..16],
                &hex[16..20],
                &hex[20..]
            );
            self.configure(MachineConfig {
                id,
                host,
                auto_connect: false,
                enabled: false,
            })?;
        }
        Ok(())
    }
    pub fn list(&self) -> Value {
        json!(
            self.store
                .lock()
                .unwrap()
                .machines
                .values()
                .collect::<Vec<_>>()
        )
    }
    pub fn configure(&self, config: MachineConfig) -> Result<(), String> {
        if config.id.is_empty()
            || config.host.is_empty()
            || config.host.starts_with('-')
            || config.host.chars().any(char::is_whitespace)
        {
            return Err("machine requires an id and a single SSH Host alias".into());
        }
        if self.configs.lock().unwrap().get(&config.id) == Some(&config) {
            return Ok(());
        }
        self.disconnect(&config.id);
        let removed: Vec<_> = self
            .configs
            .lock()
            .unwrap()
            .values()
            .filter(|c| c.host == config.host && c.id != config.id)
            .map(|c| c.id.clone())
            .collect();
        for id in removed {
            self.disconnect(&id);
            self.store.lock().unwrap().machines.remove(&id);
        }
        let mut configs = self.configs.lock().unwrap();
        configs.retain(|id, c| c.host != config.host || id == &config.id);
        configs.insert(config.id.clone(), config.clone());
        std::fs::create_dir_all(config_dir()).map_err(|e| e.to_string())?;
        let bytes = serde_json::to_vec_pretty(&configs.values().collect::<Vec<_>>())
            .map_err(|e| e.to_string())?;
        let tmp = config_dir().join("machines.json.tmp");
        std::fs::write(&tmp, bytes)
            .and_then(|_| std::fs::rename(tmp, config_dir().join("machines.json")))
            .map_err(|e| e.to_string())?;
        drop(configs);
        self.set_state(&config, "disconnected", None);
        if config.enabled && config.auto_connect {
            self.connect(&config.id)?;
        }
        Ok(())
    }
    pub fn disconnect(&self, id: &str) {
        if let Some(stop) = self.active.lock().unwrap().remove(id) {
            stop.store(true, Ordering::Release);
        }
        if let Some(config) = self.configs.lock().unwrap().get(id).cloned() {
            self.set_state(&config, "disconnected", None);
        }
    }
    pub fn connect(&self, id: &str) -> Result<(), String> {
        let config = self
            .configs
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or("unknown machine")?;
        self.disconnect(id);
        let stop = Arc::new(AtomicBool::new(false));
        self.active.lock().unwrap().insert(id.into(), stop.clone());
        let this = self.clone();
        std::thread::spawn(move || {
            let mut delay = 1;
            while !stop.load(Ordering::Acquire) {
                this.set_worker_state(&config, "connecting", None, &stop);
                match this.session(&config, &stop) {
                    Ok(()) => (),
                    Err(error) => {
                        if !stop.load(Ordering::Acquire) {
                            this.set_worker_state(&config, "offline", Some(error), &stop);
                        }
                    }
                }
                for _ in 0..delay * 10 {
                    if stop.load(Ordering::Acquire) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                delay = (delay * 2).min(30);
            }
        });
        Ok(())
    }
    fn set_worker_state(
        &self,
        config: &MachineConfig,
        state: &str,
        error: Option<String>,
        stop: &Arc<AtomicBool>,
    ) {
        let active = self.active.lock().unwrap();
        if !stop.load(Ordering::Acquire)
            && active
                .get(&config.id)
                .is_some_and(|current| Arc::ptr_eq(current, stop))
        {
            self.set_state(config, state, error);
        }
    }
    fn set_state(&self, config: &MachineConfig, state: &str, error: Option<String>) {
        let mut store = self.store.lock().unwrap();
        store.revision += 1;
        let previous = store
            .machines
            .get(&config.id)
            .and_then(|m| m.get("machineId"))
            .cloned();
        let mut value = json!({"id":config.id,"host":config.host,"autoConnect":config.auto_connect,"enabled":config.enabled,"state":state,"error":error});
        if let Some(id) = previous {
            value["machineId"] = id;
        }
        store.machines.insert(config.id.clone(), value);

        drop(store);
        self.changes.raise();
    }
    fn session(&self, config: &MachineConfig, stop: &Arc<AtomicBool>) -> Result<(), String> {
        let mut ensure = Command::new("ssh")
            .args([
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=5",
                "--",
                &config.host,
                "nerve-hub ensure",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            if stop.load(Ordering::Acquire) || Instant::now() >= until {
                let _ = ensure.kill();
                let _ = ensure.wait();
                return Err("remote ensure cancelled or timed out".into());
            }
            if let Some(status) = ensure.try_wait().map_err(|e| e.to_string())? {
                if !status.success() {
                    return Err(
                        "remote hub unavailable: authenticate SSH and install a matching nerve-hub"
                            .into(),
                    );
                }
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        drop(listener);
        let forward = format!("127.0.0.1:{port}:127.0.0.1:17890");
        let child = Command::new("ssh")
            .args([
                "-n",
                "-N",
                "-o",
                "BatchMode=yes",
                "-o",
                "ExitOnForwardFailure=yes",
                "-o",
                "ConnectTimeout=5",
                "-o",
                "ServerAliveInterval=5",
                "-o",
                "ServerAliveCountMax=2",
                "-L",
                &forward,
                "--",
                &config.host,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        struct Tunnel(std::process::Child);
        impl Drop for Tunnel {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let mut tunnel = Tunnel(child);
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .build()
            .map_err(|e| e.to_string())?;
        let endpoint = format!("http://127.0.0.1:{port}/v1/stream?localOnly=true");
        let deadline = Instant::now() + Duration::from_secs(8);
        let response = loop {
            if stop.load(Ordering::Acquire) {
                return Ok(());
            }
            if tunnel.0.try_wait().map_err(|e| e.to_string())?.is_some() {
                return Err("SSH forwarding failed".into());
            }
            match client.get(&endpoint).send() {
                Ok(response) => break response,
                Err(error) => {
                    if Instant::now() >= deadline {
                        return Err(error.to_string());
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        };
        // Cancellation drops the SSH transport, unblocking a read even when no task changes.
        let tunnel_pid = tunnel.0.id();
        let finished = Arc::new(AtomicBool::new(false));
        let cancel = stop.clone();
        let last_read = Arc::new(Mutex::new(Instant::now()));
        let last_read_watch = last_read.clone();
        let finished_watch = finished.clone();
        std::thread::spawn(move || {
            while !finished_watch.load(Ordering::Acquire) {
                if cancel.load(Ordering::Acquire)
                    || last_read_watch.lock().unwrap().elapsed() > Duration::from_secs(40)
                {
                    #[cfg(unix)]
                    {
                        let _ = Command::new("kill")
                            .args(["-TERM", &tunnel_pid.to_string()])
                            .status();
                    }
                    #[cfg(windows)]
                    {
                        let _ = Command::new("taskkill")
                            .args(["/PID", &tunnel_pid.to_string(), "/T", "/F"])
                            .status();
                    }
                    return;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        });
        struct Finish(Arc<AtomicBool>);
        impl Drop for Finish {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }
        let _finish = Finish(finished);
        for line in BufReader::new(response).lines() {
            *last_read.lock().unwrap() = Instant::now();
            if stop.load(Ordering::Acquire) {
                return Ok(());
            }
            let line = line.map_err(|e| e.to_string())?;
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let frame: Value = serde_json::from_str(data.trim()).map_err(|e| e.to_string())?;
            let mut store = self.store.lock().unwrap();
            if stop.load(Ordering::Acquire) {
                return Ok(());
            }
            apply_remote(&mut store, config, &frame)?;
            drop(store);
            self.changes.raise();
        }
        Err("remote hub stream disconnected".into())
    }
}

fn wildcard(pattern: &str, text: &str) -> bool {
    let (mut p, mut t, mut star, mut saved) = (0, 0, None, 0);
    let (pattern, text) = (pattern.as_bytes(), text.as_bytes());
    while t < text.len() {
        if p < pattern.len() && (pattern[p] == b'?' || pattern[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p < pattern.len() && pattern[p] == b'*' {
            star = Some(p);
            p += 1;
            saved = t;
        } else if let Some(s) = star {
            p = s + 1;
            saved += 1;
            t = saved;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == b'*' {
        p += 1;
    }
    p == pattern.len()
}
fn read_hosts(
    path: &std::path::Path,
    home: &std::path::Path,
    depth: usize,
    hosts: &mut Vec<String>,
) {
    if depth > 8 {
        return;
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let mut tokens = line.split_whitespace();
        let Some(key) = tokens.next() else {
            continue;
        };
        if key.eq_ignore_ascii_case("Host") {
            hosts.extend(
                tokens
                    .filter(|t| !t.starts_with('-') && !t.chars().any(|c| "*?!".contains(c)))
                    .map(str::to_owned),
            );
        } else if key.eq_ignore_ascii_case("Include") {
            for token in tokens {
                let candidate = if let Some(t) = token.strip_prefix("~/") {
                    home.join(t)
                } else {
                    let p = PathBuf::from(token);
                    if p.is_absolute() {
                        p
                    } else {
                        home.join(".ssh").join(p)
                    }
                };
                if token.contains('*') || token.contains('?') {
                    if let (Some(parent), Some(pattern)) = (
                        candidate.parent(),
                        candidate.file_name().and_then(|n| n.to_str()),
                    ) && let Ok(entries) = std::fs::read_dir(parent)
                    {
                        for entry in entries.flatten() {
                            if wildcard(pattern, &entry.file_name().to_string_lossy()) {
                                read_hosts(&entry.path(), home, depth + 1, hosts);
                            }
                        }
                    }
                } else {
                    read_hosts(&candidate, home, depth + 1, hosts);
                }
            }
        }
    }
}

/// Validate provenance before atomically replacing one origin's snapshot.
fn apply_remote(store: &mut JobStore, config: &MachineConfig, frame: &Value) -> Result<(), String> {
    if frame["protocolVersion"] != 2 {
        return Err("remote hub protocol mismatch; upgrade remote hub".into());
    }
    let origin = frame["machineId"]
        .as_str()
        .ok_or("remote machine identity missing")?;
    if origin == store.machine_id {
        return Err("remote machine resolves to this hub".into());
    }
    let mut jobs = frame["jobs"]
        .as_array()
        .cloned()
        .ok_or("remote jobs missing")?;
    for job in &mut jobs {
        if job["machineId"].as_str() != Some(origin) {
            return Err("remote stream included a relayed job".into());
        }
        let original = job["originJobId"]
            .as_str()
            .ok_or("origin job id missing")?
            .to_owned();
        job["id"] = json!(format!("{origin}/{original}"));
        job["extensions"]["sshHost"] = json!(config.host);
        if let Some(group) = job["display"]["groupId"].as_str().map(str::to_owned) {
            job["display"]["groupId"] = json!(format!("{origin}/{group}"));
            job["extensions"]["hub"] = job["display"].clone();
        }
    }
    store.revision += 1;
    store.remote_jobs.insert(origin.to_owned(), jobs);
    store.machines.insert(config.id.clone(),json!({"id":config.id,"host":config.host,"autoConnect":config.auto_connect,"enabled":config.enabled,"state":"connected","machineId":origin}));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        clock::FakeClock,
        projection::project_json,
        state::{PidProbe, PidState},
    };
    struct Dead;
    impl PidProbe for Dead {
        fn state(&self, _: i32) -> PidState {
            PidState::Dead
        }
    }
    fn store() -> JobStore {
        JobStore::new(
            Arc::new(FakeClock::new(time::macros::datetime!(2026-01-01 0:00 UTC))),
            "local".into(),
            Arc::new(Dead),
        )
    }
    fn config() -> MachineConfig {
        MachineConfig {
            id: "remote".into(),
            host: "gpu".into(),
            auto_connect: false,
            enabled: true,
        }
    }
    fn frame(origin: &str) -> Value {
        let mut job = project_json(
            json!({"id":"codex:same","producer":{"id":"codex"},"createdAt":"2025-01-01T00:00:00Z","extensions":{"pid":99999}}),
        );
        job["machineId"] = json!(origin);
        job["originJobId"] = json!("codex:same");
        json!({"protocolVersion":2,"machineId":origin,"jobs":[job]})
    }
    #[test]
    fn equal_sessions_on_different_machines_remain_distinct_and_remote_pids_survive() {
        let mut store = store();
        apply_remote(&mut store, &config(), &frame("machine-a")).unwrap();
        apply_remote(&mut store, &config(), &frame("machine-b")).unwrap();
        assert_eq!(store.jobs_json().as_array().unwrap().len(), 2);
        assert!(!store.expire_and_reap());
        store.clear();
        assert_eq!(store.jobs_json().as_array().unwrap().len(), 2);
    }
    #[test]
    fn reconnect_replaces_full_origin_snapshot_and_rejects_relay_loops_atomically() {
        let mut store = store();
        apply_remote(&mut store, &config(), &frame("remote-a")).unwrap();
        let mut invalid = frame("remote-a");
        invalid["jobs"][0]["machineId"] = json!("third-party");
        assert!(apply_remote(&mut store, &config(), &invalid).is_err());
        assert_eq!(store.jobs_json().as_array().unwrap().len(), 1);
        let own = store.machine_id.clone();
        assert!(apply_remote(&mut store, &config(), &frame(&own)).is_err());
        let empty = json!({"protocolVersion":2,"machineId":"remote-a","jobs":[]});
        apply_remote(&mut store, &config(), &empty).unwrap();
        assert!(store.jobs_json().as_array().unwrap().is_empty());
    }
}
