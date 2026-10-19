//! Application state for the sidebar TUI.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use time::OffsetDateTime;

use nerve_surface_core::filter::StatusFilter;
use nerve_surface_core::frame::JobView;
use nerve_surface_core::group::{MachineSection, group_by_machine};
use nerve_surface_core::jobpath::job_path;

use crate::git::GitSnapshot;
use crate::preview::PanePreview;
use nerve_surface_core::store::{JobsSnapshot, JobsStore};

/// Floor between two `git` snapshots on the loop tick. A snapshot is five
/// subprocesses; the Git panel does not need them five times a second.
const GIT_MIN_INTERVAL: Duration = Duration::from_secs(2);

/// What the panel under the job list is showing.
///
/// `Prompt` — not an activity log: the row above already says what the agent is
/// doing, and the hub's own timeline is heartbeat noise. What a monitor cannot
/// see from the row is *what this agent was asked for*.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BottomTab {
    Prompt,
    Git,
}

pub struct AppState {
    pub store: JobsStore,
    pub status_filter: StatusFilter,
    /// Row the cursor is on. Derived from [`Self::selected_id`] after every
    /// rebuild — never the other way round.
    pub selected_index: usize,
    pub bottom_tab: BottomTab,
    pub bottom_scroll: u16,
    /// Panel folded to its title line, so a long list gets the whole pane.
    pub bottom_collapsed: bool,
    pub git_snapshot: GitSnapshot,
    pub now: OffsetDateTime,
    pub bottom_height: u16,
    /// Last known pane size, so wrapped content can be measured before it is
    /// drawn (scroll bounds) instead of guessed.
    pub viewport: (u16, u16),
    pub preview: PanePreview,
    /// Job the cursor is anchored to.
    ///
    /// The cursor holds a *job*, not a row. The list re-sorts on every frame
    /// (attention, health, `updatedAt` — `compare_jobs`), so a job arriving or
    /// merely reporting activity slides the rows under a cursor that never
    /// moved. Anchoring by index meant the selection — and with it the preview
    /// swap and the Git panel — quietly changed to a different job.
    selected_id: Option<String>,
    snapshot: Arc<JobsSnapshot>,
    sections: Vec<MachineSection>,
    /// Flat selection order — every section's jobs, in section order.
    visible: Vec<usize>,
    /// Path `git_snapshot` describes; `None` means it needs recomputing.
    git_path: Option<PathBuf>,
    git_checked: Instant,
}

impl AppState {
    pub fn new(store: JobsStore, bottom_height: u16) -> Self {
        Self::with_preview(store, bottom_height, PanePreview::new())
    }

    /// The same state over a preview the caller supplies — the seam unit tests
    /// use to keep a test run out of the real tmux server
    /// ([`PanePreview::detached`]).
    fn with_preview(store: JobsStore, bottom_height: u16, preview: PanePreview) -> Self {
        Self {
            store,
            status_filter: StatusFilter::All,
            selected_index: 0,
            selected_id: None,
            bottom_tab: BottomTab::Prompt,
            bottom_scroll: 0,
            bottom_collapsed: false,
            git_snapshot: GitSnapshot::default(),
            now: OffsetDateTime::now_utc(),
            bottom_height,
            viewport: (0, 0),
            preview,
            snapshot: Arc::new(JobsSnapshot::default()),
            sections: Vec::new(),
            visible: Vec::new(),
            git_path: None,
            git_checked: Instant::now(),
        }
    }

    pub fn jobs(&self) -> &[JobView] {
        &self.snapshot.jobs
    }

    pub fn hub_error(&self) -> Option<&str> {
        self.snapshot.error.as_deref()
    }
    pub fn offline(&self) -> bool {
        self.snapshot.offline
    }

    /// Sections grouped by machine alias (default layout, aligned with macOS).
    pub fn sections(&self) -> &[MachineSection] {
        &self.sections
    }

    pub fn job_at(&self, index: usize) -> Option<&JobView> {
        self.snapshot.jobs.get(index)
    }

    /// How many jobs the current filter leaves selectable.
    pub fn visible_len(&self) -> usize {
        self.visible.len()
    }

    /// Take whatever the reader thread has published and re-derive the view.
    ///
    /// The loop calls this several times a second whether or not a frame
    /// arrived, so the expensive half — grouping, sorting, re-anchoring the
    /// cursor — runs only when the store actually handed over a new snapshot.
    /// The reader publishes a fresh `Arc` per frame and per offline flip, so
    /// pointer identity is the whole test. The cheap half still runs every
    /// tick: `now` ages the rows, and the pane follow / preview / git calls
    /// carry throttles of their own.
    ///
    /// Returns `true` when the drawn contents changed (new snapshot, or the
    /// highlight followed a pane the human walked into) so the loop can skip
    /// a ratatui pass on idle ticks.
    pub fn refresh_from_hub(&mut self) -> bool {
        let next = self.store.snapshot();
        let arrived = !Arc::ptr_eq(&next, &self.snapshot);
        self.snapshot = next;
        self.now = OffsetDateTime::now_utc();
        if arrived {
            self.rebuild_sections();
            // The row the cursor was on may now belong to another job; the job
            // it was on is what it keeps.
            self.restore_selection(self.selected_index);
        }
        let followed = self.follow_focused_pane();
        self.sync_preview();
        self.refresh_git_if_due();
        arrived || followed
    }

    /// Group, sort and flatten the current snapshot once. Every reader below
    /// works off the cached result instead of redoing this per call.
    fn rebuild_sections(&mut self) {
        let snapshot = Arc::clone(&self.snapshot);
        self.sections = group_by_machine(&snapshot.jobs, self.now, self.status_filter);
        self.visible = self
            .sections
            .iter()
            .flat_map(|section| section.jobs.iter().copied())
            .collect();
    }

    pub fn selected_job(&self) -> Option<&JobView> {
        self.visible
            .get(self.selected_index)
            .and_then(|index| self.snapshot.jobs.get(*index))
    }

    /// Switch the filter bar and re-derive everything that depends on it.
    ///
    /// A selection the new filter still shows is kept — cycling filters to look
    /// around and coming back must not lose the job you were on. One the filter
    /// hides falls to the top of the list.
    pub fn set_filter(&mut self, filter: StatusFilter) {
        self.status_filter = filter;
        self.rebuild_sections();
        self.restore_selection(0);
        self.invalidate_git();
    }

    pub fn set_bottom_tab(&mut self, tab: BottomTab) {
        self.bottom_tab = tab;
        self.bottom_scroll = 0;
        if tab == BottomTab::Git {
            self.refresh_git();
        }
    }

    pub fn toggle_bottom_tab(&mut self) {
        self.set_bottom_tab(match self.bottom_tab {
            BottomTab::Prompt => BottomTab::Git,
            BottomTab::Git => BottomTab::Prompt,
        });
    }

    /// Fold the panel away, or bring it back where it was.
    pub fn toggle_bottom_collapsed(&mut self) {
        self.bottom_collapsed = !self.bottom_collapsed;
        self.bottom_scroll = 0;
    }

    pub fn set_viewport(&mut self, width: u16, height: u16) {
        self.viewport = (width, height);
    }

    /// Rows the panel occupies inside a pane `total` rows tall.
    ///
    /// Collapsed it keeps its title line: a panel that vanished completely
    /// would leave nothing to click, and no hint that it exists.
    pub fn bottom_rows(&self, total: u16) -> u16 {
        if self.bottom_collapsed {
            return 1;
        }
        self.bottom_height.min(total.saturating_sub(4))
    }

    /// What the Prompt panel shows for the selection, if anything.
    pub fn prompt_text(&self) -> Option<&str> {
        self.selected_job()
            .and_then(|job| job.extensions.last_prompt.as_deref())
            .map(str::trim)
            .filter(|prompt| !prompt.is_empty())
    }

    /// Scroll the detail panel. The selection never moves — the panel describes
    /// the selected job, it is not a second place to stand.
    pub fn scroll_bottom(&mut self, delta: i32) {
        let max = self.bottom_max_scroll() as i32;
        let next = (self.bottom_scroll as i32 + delta).clamp(0, max);
        self.bottom_scroll = next as u16;
    }

    /// Last scroll offset that still shows content, so the panel cannot be
    /// scrolled into blankness.
    fn bottom_max_scroll(&self) -> u16 {
        let (width, height) = self.viewport;
        let visible = self.bottom_rows(height).saturating_sub(1);
        let lines = match self.bottom_tab {
            BottomTab::Prompt => self
                .prompt_text()
                .map(|prompt| wrapped_lines(prompt, width))
                .unwrap_or(1),
            BottomTab::Git => self.git_snapshot.line_count(),
        };
        (lines as u16).saturating_sub(visible)
    }

    pub fn move_selection(&mut self, delta: isize) {
        let count = self.visible.len();
        if count == 0 {
            self.selected_index = 0;
            self.selected_id = None;
            return;
        }
        let next = self.selected_index as isize + delta;
        self.selected_index = next.clamp(0, count as isize - 1) as usize;
        self.selected_id = self.job_id_at_cursor();
        self.bottom_scroll = 0;
        self.invalidate_git();
    }

    /// Move the cursor onto whatever job the human just walked into.
    ///
    /// The mirror image of the preview: the highlight moves and the pane
    /// follows, so a pane the human focuses themselves should bring the
    /// highlight to it. Identity again — the pids on that pane's terminal, one
    /// of which the producer reported.
    fn follow_focused_pane(&mut self) -> bool {
        let Some(pids) = self.preview.newly_focused_pids() else {
            return false;
        };
        let Some(id) = job_running_as(&self.snapshot.jobs, &pids) else {
            return false;
        };
        // Already there: never fight a cursor that agrees.
        if self.selected_id.as_deref() == Some(id.as_str()) {
            return false;
        }
        self.selected_id = Some(id);
        self.restore_selection(self.selected_index);
        self.bottom_scroll = 0;
        self.invalidate_git();
        true
    }

    /// Mirror the selection into the sibling pane, cloning the job only when
    /// the selection actually moved.
    ///
    /// Called once per loop tick rather than per keypress: a swap is four tmux
    /// round-trips, and holding `j` must not fire one per row crossed.
    pub fn sync_preview(&mut self) {
        let unchanged = match self.selected_job() {
            Some(job) => self.preview.is_showing(job),
            None => self.preview.is_idle(),
        };
        if unchanged {
            return;
        }
        match self.selected_job().cloned() {
            Some(job) => self.preview.show(&job),
            None => self.preview.restore(),
        }
    }

    /// Put the cursor back on the job it was holding after the list changed.
    ///
    /// `fallback` is the row to take when that job is not in the list any more
    /// (it ended, or a filter hides it) — the closest thing to where the cursor
    /// was standing.
    pub fn restore_selection(&mut self, fallback: usize) {
        let count = self.visible.len();
        if count == 0 {
            self.selected_index = 0;
            self.selected_id = None;
            return;
        }
        self.selected_index = self.anchor_row().unwrap_or_else(|| fallback.min(count - 1));
        self.selected_id = self.job_id_at_cursor();
    }

    /// Row holding the anchored job, if the list still has it.
    fn anchor_row(&self) -> Option<usize> {
        let anchor = self.selected_id.as_deref()?;
        self.visible.iter().position(|index| {
            self.snapshot
                .jobs
                .get(*index)
                .is_some_and(|job| job.id == anchor)
        })
    }

    fn job_id_at_cursor(&self) -> Option<String> {
        self.selected_job().map(|job| job.id.clone())
    }

    fn git_target(&self) -> PathBuf {
        self.selected_job()
            .and_then(job_path)
            .unwrap_or_else(|| PathBuf::from("."))
    }

    fn invalidate_git(&mut self) {
        self.git_path = None;
    }

    pub fn refresh_git(&mut self) {
        let path = self.git_target();
        self.git_snapshot = crate::git::snapshot_for_path(&path);
        self.git_path = Some(path);
        self.git_checked = Instant::now();
    }

    /// Tick-path refresh: only while the Git panel is showing, and only when
    /// the selection moved or the snapshot has gone stale.
    fn refresh_git_if_due(&mut self) {
        if self.bottom_tab != BottomTab::Git {
            return;
        }
        let fresh = self.git_checked.elapsed() < GIT_MIN_INTERVAL;
        if fresh && self.git_path.as_deref() == Some(self.git_target().as_path()) {
            return;
        }
        self.refresh_git();
    }

    /// Map a row index inside the job list (0 = first line below the filter bar) to
    /// a flat job index. Section headers occupy a row but are not selectable.
    pub fn global_index_at_list_row(&self, row: usize) -> Option<usize> {
        let mut line = 0usize;
        let mut global = 0usize;
        for section in &self.sections {
            line += 1; // section header
            for _ in &section.jobs {
                if line == row {
                    return Some(global);
                }
                line += 1;
                global += 1;
            }
        }
        None
    }

    pub fn select_global_index(&mut self, index: usize) {
        let count = self.visible.len();
        if count == 0 {
            self.selected_index = 0;
            self.selected_id = None;
            return;
        }
        self.selected_index = index.min(count - 1);
        self.selected_id = self.job_id_at_cursor();
        self.bottom_scroll = 0;
        self.invalidate_git();
    }
}

/// Rows `text` needs once wrapped to `width` columns.
pub fn wrapped_lines(text: &str, width: u16) -> usize {
    let width = (width as usize).max(1);
    text.lines()
        .map(|line| crate::columns::display_width(line).div_ceil(width).max(1))
        .sum::<usize>()
        .max(1)
}

/// Id of the job whose producer process is one of `pids`.
///
/// A pane runs a shell and whatever it launched, so several pids share one
/// terminal; exactly one of them is what a producer reported as its own.
fn job_running_as(jobs: &[JobView], pids: &[u32]) -> Option<String> {
    jobs.iter()
        .find(|job| job.extensions.pid.is_some_and(|pid| pids.contains(&pid)))
        .map(|job| job.id.clone())
}

#[cfg(test)]
mod panel_tests {
    use super::*;

    fn state_with(jobs: serde_json::Value) -> AppState {
        let store = JobsStore::new();
        store.set_jobs(nerve_surface_core::testkit::jobs(jobs));
        let mut state = AppState::with_preview(store, 8, PanePreview::detached());
        state.set_viewport(30, 24);
        state.refresh_from_hub();
        state
    }

    /// Re-publish the hub's list, in the order the hub would send it.
    fn arrive(state: &mut AppState, jobs: serde_json::Value) {
        state
            .store
            .set_jobs(nerve_surface_core::testkit::jobs(jobs));
        state.refresh_from_hub();
    }

    fn selected(state: &AppState) -> Option<&str> {
        state.selected_job().map(|job| job.id.as_str())
    }

    fn job(id: &str, updated: &str) -> serde_json::Value {
        serde_json::json!({ "id": id, "name": id, "updatedAt": updated })
    }

    /// Characterization of the grouping and ordering rule, pinned before it
    /// moved to `nerve-surface-core`.
    ///
    /// The fixture is built around the one case that breaks if `now` is
    /// resampled per comparison instead of captured once: two jobs that carry
    /// no `updatedAt` both fall back to `now`, tie there, and are then ordered
    /// by `createdAt`. Give them different `now` values and the tie never
    /// happens, `createdAt` is never consulted, and the order silently follows
    /// the clock.
    #[test]
    fn sections_group_by_machine_and_order_within_each() {
        let state = state_with(serde_json::json!([
            { "id": "b-old", "name": "b-old", "alias": "Beta",
              "createdAt": "2026-07-19T08:00:00Z" },
            { "id": "a-stale", "name": "a-stale", "alias": "alpha",
              "updatedAt": "2020-01-01T00:00:00Z" },
            { "id": "a-new", "name": "a-new", "alias": "alpha",
              "createdAt": "2026-07-19T09:00:00Z" },
            { "id": "a-old", "name": "a-old", "alias": "alpha",
              "createdAt": "2026-07-19T08:00:00Z" },
            { "id": "b-loud", "name": "b-loud", "alias": "Beta",
              "attention": { "level": "required" },
              "createdAt": "2020-01-01T00:00:00Z" },
        ]));

        let shape: Vec<(&str, Vec<&str>)> = state
            .sections()
            .iter()
            .map(|section| {
                (
                    section.title.as_str(),
                    section
                        .jobs
                        .iter()
                        .map(|index| state.job_at(*index).expect("job in section").id.as_str())
                        .collect(),
                )
            })
            .collect();

        assert_eq!(
            shape,
            vec![
                // Titles sort case-insensitively, so `alpha` precedes `Beta`.
                ("alpha", vec!["a-new", "a-old", "a-stale"]),
                // Attention outranks every time field.
                ("Beta", vec!["b-loud", "b-old"]),
            ]
        );
    }

    #[test]
    fn panel_shows_what_the_human_asked_for() {
        let state = state_with(serde_json::json!([{
            "id": "claude-code:s1",
            "name": "nerve",
            "current": { "type": "tool", "summary": "Using Bash" },
            "extensions": { "lastPrompt": "fix the sidebar preview" }
        }]));
        assert_eq!(state.prompt_text(), Some("fix the sidebar preview"));
    }

    #[test]
    fn a_job_without_a_prompt_shows_none_rather_than_its_activity() {
        let state = state_with(serde_json::json!([{
            "id": "codex:s1",
            "name": "index",
            "current": { "type": "tool", "summary": "Using Bash" }
        }]));
        assert_eq!(state.prompt_text(), None);
    }

    #[test]
    fn blank_prompts_are_not_prompts() {
        let state = state_with(serde_json::json!([{
            "id": "claude-code:s1",
            "extensions": { "lastPrompt": "   " }
        }]));
        assert_eq!(state.prompt_text(), None);
    }

    #[test]
    fn collapsing_leaves_the_title_line_and_gives_the_rest_to_the_list() {
        let mut state = state_with(serde_json::json!([]));
        assert_eq!(state.bottom_rows(24), 8);
        state.toggle_bottom_collapsed();
        assert_eq!(state.bottom_rows(24), 1);
        state.toggle_bottom_collapsed();
        assert_eq!(state.bottom_rows(24), 8);
    }

    #[test]
    fn a_short_pane_never_gives_the_panel_more_than_it_has() {
        let state = state_with(serde_json::json!([]));
        assert_eq!(state.bottom_rows(10), 6);
    }

    #[test]
    fn scrolling_stops_at_the_end_of_the_content() {
        let mut state = state_with(serde_json::json!([{
            "id": "claude-code:s1",
            "extensions": { "lastPrompt": "short" }
        }]));
        state.scroll_bottom(20);
        assert_eq!(state.bottom_scroll, 0);
        state.scroll_bottom(-20);
        assert_eq!(state.bottom_scroll, 0);
    }

    #[test]
    fn a_long_prompt_scrolls_by_its_wrapped_height() {
        let mut state = state_with(serde_json::json!([{
            "id": "claude-code:s1",
            "extensions": { "lastPrompt": "x".repeat(300) }
        }]));
        // 300 columns of text in a 30-column pane is 10 rows; 7 of them show.
        state.scroll_bottom(20);
        assert_eq!(state.bottom_scroll, 3);
    }

    #[test]
    fn the_cursor_finds_the_job_running_on_a_pane() {
        let jobs: Vec<JobView> = nerve_surface_core::testkit::jobs(serde_json::json!([
            { "id": "claude-code:s1", "extensions": { "pid": 4242 } },
            { "id": "codex:s2", "extensions": { "pid": 77 } },
            { "id": "claude-code:s3" },
        ]));

        // A pane runs a shell, the agent, and whatever the agent spawned.
        assert_eq!(
            job_running_as(&jobs, &[900, 77, 1201]),
            Some("codex:s2".to_string())
        );
        assert_eq!(job_running_as(&jobs, &[900, 1201]), None);
        // A job that never reported a pid cannot be found this way.
        assert_eq!(job_running_as(&jobs, &[]), None);
    }

    #[test]
    fn a_job_arriving_does_not_slide_another_job_under_the_cursor() {
        let mut state = state_with(serde_json::json!([
            job("older", "2026-08-23T09:00:00Z"),
            job("newer", "2026-08-23T10:00:00Z"),
        ]));
        // Rows are newest-first, so the cursor starts on `newer`.
        state.move_selection(1);
        assert_eq!(selected(&state), Some("older"));

        // A third job arrives and takes the top row; `older` reports activity
        // and overtakes `newer`. Every row index the cursor could have held now
        // means a different job.
        arrive(
            &mut state,
            serde_json::json!([
                job("older", "2026-08-23T10:30:00Z"),
                job("newer", "2026-08-23T10:00:00Z"),
                job("arrived", "2026-08-23T11:00:00Z"),
            ]),
        );
        assert_eq!(selected(&state), Some("older"));
    }

    #[test]
    fn a_job_that_leaves_hands_the_cursor_to_the_row_it_held() {
        let mut state = state_with(serde_json::json!([
            job("first", "2026-08-23T11:00:00Z"),
            job("second", "2026-08-23T10:00:00Z"),
            job("third", "2026-08-23T09:00:00Z"),
        ]));
        state.move_selection(1);
        assert_eq!(selected(&state), Some("second"));

        arrive(
            &mut state,
            serde_json::json!([
                job("first", "2026-08-23T11:00:00Z"),
                job("third", "2026-08-23T09:00:00Z"),
            ]),
        );
        assert_eq!(selected(&state), Some("third"));
    }

    #[test]
    fn the_cursor_survives_a_filter_that_still_shows_its_job() {
        let mut state = state_with(serde_json::json!([
            job("first", "2026-08-23T11:00:00Z"),
            job("second", "2026-08-23T10:00:00Z"),
        ]));
        state.move_selection(1);
        assert_eq!(selected(&state), Some("second"));

        state.set_filter(StatusFilter::All);
        assert_eq!(selected(&state), Some("second"));
    }

    #[test]
    fn wrapped_lines_counts_rows_not_characters() {
        assert_eq!(wrapped_lines("", 20), 1);
        assert_eq!(wrapped_lines("short", 20), 1);
        assert_eq!(wrapped_lines(&"x".repeat(41), 20), 3);
        assert_eq!(wrapped_lines("one\ntwo", 20), 2);
    }
}
