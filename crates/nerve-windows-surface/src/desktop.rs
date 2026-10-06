//! Versioned presentation boundary for native desktop hosts. No GUI dependency.
use crate::{
    flyout::sections,
    settings::Settings,
    tray::{
        icon::{render, Theme},
        view,
    },
};
use base64::{engine::general_purpose::STANDARD, Engine};
use nerve_surface_core::{
    display::{activity_text, age_label},
    filter::StatusFilter,
    palette,
    status::StatusClass,
    store::JobsSnapshot,
    tally::Tally,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub const VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Command {
    Quit,
    CycleGroup,
    Preferences { settings: Settings },
    Open { id: String },
    Copy { id: String },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub id: String,
    pub name: String,
    pub alias: String,
    pub status: &'static str,
    pub color: String,
    pub activity: String,
    pub age: String,
    pub prompt: String,
    pub workspace: String,
    pub location: String,
    pub recent: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Section {
    pub title: String,
    pub jobs: Vec<Row>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Presentation {
    pub version: u32,
    pub r#type: &'static str,
    pub offline: bool,
    pub running: usize,
    pub attention: usize,
    pub settings: Settings,
    pub sections: Vec<Section>,
    pub tooltip: String,
    pub icon_size: u32,
    pub icon_rgba: String,
}

pub fn project(
    snapshot: &JobsSnapshot,
    settings: &Settings,
    theme: Theme,
    now: OffsetDateTime,
) -> Presentation {
    let tally = Tally::of(&snapshot.jobs);
    let tray = view::of(snapshot, true, theme, 32, now);
    let sections = sections::of(&snapshot.jobs, settings.group_mode, StatusFilter::All, now)
        .into_iter()
        .map(|section| Section {
            title: section.title,
            jobs: section
                .jobs
                .into_iter()
                .filter_map(|index| snapshot.jobs.get(index))
                .map(|job| {
                    let status = StatusClass::of(job);
                    Row {
                        id: job.id.clone(),
                        name: job.name.clone(),
                        alias: job.alias.clone(),
                        status: status.label(),
                        color: palette::color_of(status).hex(),
                        activity: activity_text(job).to_string(),
                        age: age_label(now, job),
                        prompt: job.extensions.last_prompt.clone().unwrap_or_default(),
                        workspace: job.context.workspace.clone().unwrap_or_default(),
                        location: job
                            .location
                            .as_ref()
                            .and_then(|loc| loc.focus_hint.as_ref().or(loc.open_url.as_ref()))
                            .cloned()
                            .unwrap_or_default(),
                        recent: job
                            .timeline
                            .iter()
                            .rev()
                            .take(5)
                            .map(|entry| {
                                let clock = entry
                                    .at
                                    .map(|at| {
                                        format!(
                                            "{:02}:{:02}",
                                            at.instant().hour(),
                                            at.instant().minute()
                                        )
                                    })
                                    .unwrap_or_default();
                                format!("{clock}  {}", entry.title)
                            })
                            .collect(),
                    }
                })
                .collect(),
        })
        .collect();
    Presentation {
        version: VERSION,
        r#type: "frame",
        offline: snapshot.offline,
        running: tally.running + tally.monitor,
        attention: tally.problem + tally.attention + tally.waiting,
        settings: settings.clone(),
        sections,
        tooltip: tray.tooltip,
        icon_size: tray.icon.size,
        icon_rgba: STANDARD.encode(render(&tray.icon)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presentation_preserves_structured_status_and_unicode_details() {
        let jobs = serde_json::from_value(serde_json::json!([
            {"id":"a", "name":"构建", "current":{"type":"tool", "summary":"failed in prose"}, "extensions":{"lastPrompt":"修复它"}},
            {"id":"b", "name":"Review", "attention":{"level":"required", "reason":"approval"}}
        ])).unwrap();
        let frame = project(
            &JobsSnapshot {
                jobs,
                ..Default::default()
            },
            &Settings::default(),
            Theme::Dark,
            OffsetDateTime::now_utc(),
        );
        assert_eq!(frame.running, 1);
        assert_eq!(frame.attention, 1);
        let row = frame
            .sections
            .iter()
            .flat_map(|s| &s.jobs)
            .find(|j| j.id == "a")
            .unwrap();
        assert_eq!(row.status, "running");
        assert_eq!(row.prompt, "修复它");
        assert_eq!(
            STANDARD.decode(&frame.icon_rgba).unwrap().len(),
            32 * 32 * 4
        );
        let json = serde_json::to_value(frame).unwrap();
        assert_eq!(json["version"], VERSION);
        assert_eq!(json["type"], "frame");
    }
    #[test]
    fn unknown_commands_are_rejected_instead_of_controlling_jobs() {
        assert!(serde_json::from_str::<Command>(r#"{"type":"approve","id":"a"}"#).is_err());
        assert!(serde_json::from_str::<Command>(r#"{"type":"open"}"#).is_err());
    }
}
