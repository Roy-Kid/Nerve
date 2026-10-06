//! Native UI subprocess: stdin commands, stdout NDJSON. EOF releases the hub.
use nerve_surface_core::{machine, store::JobsStore, stream::stream_path};
use nerve_windows_surface::{
    actions::{self, clipboard::SystemClipboard, shell::SystemOpener},
    desktop::{self, Command},
    notify::policy::{AskPolicy, Settings as NotifySettings},
    platform::theme,
    settings::{settings_path, Settings},
};
use serde_json::json;
use std::{
    io::{self, BufRead, Write},
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};

fn emit(value: &impl serde::Serialize) -> io::Result<()> {
    let mut out = io::stdout().lock();
    serde_json::to_writer(&mut out, value)?;
    writeln!(out)?;
    out.flush()
}

fn main() -> io::Result<()> {
    nerve_surface_core::log::init("nerve-windows-core");
    let store = JobsStore::new();
    let args: Vec<String> = std::env::args().collect();
    let fixture = args
        .iter()
        .position(|arg| arg == "--fixture")
        .and_then(|at| args.get(at + 1));
    if let Some(path) = fixture {
        let raw = std::fs::read_to_string(path)?;
        let value: serde_json::Value = serde_json::from_str(&raw)?;
        let jobs = value.get("jobs").cloned().unwrap_or(value);
        store.set_jobs(serde_json::from_value(jobs)?);
    } else {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_default();
        store.clone().spawn_reader(home, stream_path("windows"));
    }
    let (send, receive) = mpsc::sync_channel(32);
    std::thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if send.send(line).is_err() {
                return;
            }
        }
        let _ = send.send(r#"{"type":"quit"}"#.into());
    });
    let mut settings = if fixture.is_some() {
        Settings::default()
    } else {
        Settings::load(&settings_path())
    };
    let mut policy = AskPolicy::new();
    let mut last = String::new();
    loop {
        match receive.recv_timeout(Duration::from_millis(250)) {
            Ok(line) => match serde_json::from_str::<Command>(&line) {
                Ok(Command::Quit) => return Ok(()),
                Ok(Command::CycleGroup) => {
                    settings.group_mode = settings.group_mode.next();
                    if fixture.is_none() {
                        save(&settings);
                    }
                }
                Ok(Command::Preferences { settings: next }) => {
                    settings = next.clamped();
                    if fixture.is_none() {
                        save(&settings);
                    }
                }
                Ok(command @ (Command::Open { .. } | Command::Copy { .. })) => {
                    let (id, copy) = match &command {
                        Command::Open { id } => (id, false),
                        Command::Copy { id } => (id, true),
                        _ => unreachable!(),
                    };
                    let snapshot = store.snapshot();
                    if let Some(job) = snapshot.jobs.iter().find(|job| &job.id == id) {
                        let outcome = if copy {
                            actions::copy(job, &mut SystemClipboard)
                        } else {
                            actions::perform(
                                job,
                                machine::local_alias(),
                                &SystemOpener,
                                &mut SystemClipboard,
                            )
                        };
                        emit(
                            &json!({"version": desktop::VERSION, "type":"action", "opened":outcome.opened, "message":outcome.message}),
                        )?;
                    }
                }
                Err(_) => emit(
                    &json!({"version":desktop::VERSION,"type":"error","message":"Unsupported desktop command"}),
                )?,
            },
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        let snapshot = store.snapshot();
        let frame = desktop::project(
            &snapshot,
            &settings,
            theme::current(),
            time::OffsetDateTime::now_utc(),
        );
        let encoded = serde_json::to_string(&frame)?;
        if encoded != last {
            emit(&frame)?;
            last = encoded;
        }
        for toast in policy.evaluate(
            &snapshot.jobs,
            NotifySettings {
                enabled: settings.toasts_enabled
                    && !snapshot.offline
                    && snapshot.notify.may_interrupt("windows"),
                sound: settings.toast_sound,
                floor: settings.toast_floor,
            },
            Instant::now(),
        ) {
            emit(
                &json!({"version":desktop::VERSION, "type":"toast", "id":toast.job_id,"title":toast.title,"body":toast.body,"sound":toast.sound}),
            )?;
        }
    }
}

fn save(settings: &Settings) {
    if let Err(error) = settings.save(&settings_path()) {
        tracing::warn!(%error, "could not save desktop preferences");
    }
}
