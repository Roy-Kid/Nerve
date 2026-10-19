//! The one process-starting entry point used by all surfaces.
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub fn probe() -> Result<bool, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(500))
        .build()
        .map_err(|e| e.to_string())?;
    let response = match client.get("http://127.0.0.1:17890/v1/health").send() {
        Ok(response) => response,
        Err(error) if error.is_connect() => return Ok(false),
        Err(error) => return Err(format!("hub health probe failed: {error}")),
    };
    let bytes = response.bytes().map_err(|e| e.to_string())?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| "port 17890 is occupied by another service")?;
    if value["service"] != "nerve" || value["protocolVersion"] != 2 || value["ok"] != true {
        return Err("port 17890 is occupied by another service or an older Nerve hub; upgrade and restart it".into());
    }
    Ok(true)
}

pub fn ensure() -> Result<(), String> {
    if probe()? {
        return Ok(());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut command = Command::new(exe);
    command
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x00000008 | 0x00000200);
    }
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if probe()? {
            // Reap the bootstrap child after a competing hub wins, without owning the daemon.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            return Ok(());
        }
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            if probe()? {
                return Ok(());
            }
            return Err(format!("hub exited before readiness: {status}"));
        }
        if Instant::now() >= deadline {
            return Err("hub did not become ready within five seconds".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
