//! Bootstrap adapter. Singleton probing, spawning and readiness belong to ensure.
use crate::hub::Hub;
use std::{
    fmt,
    path::Path,
    process::{Command, Stdio},
};
pub trait HealthProbe {
    fn is_serving(&self) -> bool;
}
pub trait ProcessSpawner {
    fn spawn(&mut self, program: &Path, args: &[&str]) -> Result<(), SpawnError>;
}
#[derive(Debug)]
pub struct SpawnError(String);
impl SpawnError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}
impl fmt::Display for SpawnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for SpawnError {}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Launch {
    AlreadyServing,
    Spawned,
    Missing,
    Failed,
}
impl Launch {
    pub fn is_offline(self) -> bool {
        matches!(self, Self::Missing | Self::Failed)
    }
}
pub struct HubLauncher<S: ProcessSpawner, H: HealthProbe> {
    spawner: S,
    health: H,
    pub last_error: Option<String>,
}
impl<S: ProcessSpawner, H: HealthProbe> HubLauncher<S, H> {
    pub fn new(spawner: S, health: H) -> Self {
        Self {
            spawner,
            health,
            last_error: None,
        }
    }
    pub fn ensure(&mut self, hub: Option<&Path>) -> Launch {
        self.last_error = None;
        let Some(program) = hub else {
            return if self.health.is_serving() {
                Launch::AlreadyServing
            } else {
                Launch::Missing
            };
        };
        match self.spawner.spawn(program, &["ensure"]) {
            Ok(()) => Launch::Spawned,
            Err(e) => {
                self.last_error = Some(e.to_string());
                Launch::Failed
            }
        }
    }
}
pub struct HttpHealth {
    hub: Hub,
}
impl HttpHealth {
    pub fn new(hub: Hub) -> Self {
        Self { hub }
    }
}
impl HealthProbe for HttpHealth {
    fn is_serving(&self) -> bool {
        self.hub
            .get("/v1/health")
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .is_some_and(|v| v["service"] == "nerve" && v["protocolVersion"] == 2)
    }
}
pub struct DetachedSpawner;
impl DetachedSpawner {
    pub fn new() -> Self {
        Self
    }
}
impl Default for DetachedSpawner {
    fn default() -> Self {
        Self
    }
}
impl ProcessSpawner for DetachedSpawner {
    fn spawn(&mut self, program: &Path, args: &[&str]) -> Result<(), SpawnError> {
        let output = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| SpawnError::new(e.to_string()))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(SpawnError::new(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ))
        }
    }
}
