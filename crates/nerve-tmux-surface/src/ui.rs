//! Sidebar rendering.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::colors;
use nerve_surface_core::display::{activity_text, age_label, cell_text};

use crate::columns::{self, truncate};
use crate::icons;
use crate::state::{AppState, BottomTab};
use nerve_surface_core::filter::StatusFilter;
use nerve_surface_core::frame::JobView;
use nerve_surface_core::status::StatusClass;

const AGE_COL: usize = 4;

pub fn draw(frame: &mut Frame, state: &AppState) {
    let area = frame.area();
    let bottom_h = state.bottom_rows(area.height);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(bottom_h),
        ])
        .split(area);

    draw_filter_row(frame, chunks[0], state);
    draw_job_list(frame, chunks[1], state);
    draw_bottom(frame, chunks[2], state);
}

fn draw_filter_row(frame: &mut Frame, area: Rect, state: &AppState) {
    let counts = StatusFilter::counts(state.jobs());
    let mut spans = Vec::new();
    for (idx, filter) in StatusFilter::ALL.iter().enumerate() {
        if idx > 0 {
            spans.push(Span::raw(" "));
        }
        let active = state.status_filter == *filter;
        let count = counts[idx];
        let color = Color::Indexed(if count == 0 && !active {
            colors::FILTER_INACTIVE
        } else {
            status_filter_color(*filter)
        });
        let mut style = Style::default().fg(color);
        if active {
            style = style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
        }
        spans.push(Span::styled(
            format!("{}{count}", icons::filter_icon(*filter)),
            style,
        ));
    }
    if state.offline() {
        spans.push(Span::styled(
            " offline",
            Style::default()
                .fg(Color::Indexed(colors::ERROR))
                .add_modifier(Modifier::ITALIC),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_job_list(frame: &mut Frame, area: Rect, state: &AppState) {
    let sections = state.sections();
    let width = area.width.max(8) as usize;
    let mut lines = Vec::new();
    if sections.is_empty() {
        lines.push(Line::from(Span::styled(
            if state.offline() {
                state.hub_error().unwrap_or("offline")
            } else {
                "no jobs"
            },
            Style::default().fg(Color::Indexed(colors::TEXT_MUTED)),
        )));
    } else {
        let budget = name_budget(width);
        let mut global_idx = 0usize;

        for section in sections {
            lines.push(Line::from(Span::styled(
                truncate(&section.title, width.saturating_sub(1)).into_owned(),
                Style::default()
                    .fg(Color::Indexed(colors::BRANCH))
                    .add_modifier(Modifier::BOLD),
            )));

            let jobs: Vec<&JobView> = section
                .jobs
                .iter()
                .filter_map(|index| state.job_at(*index))
                .collect();
            let rows: Vec<[String; 2]> = jobs
                .iter()
                .map(|job| {
                    [
                        truncate(&job.name, budget).into_owned(),
                        truncate(cell_text(activity_text(job)), budget).into_owned(),
                    ]
                })
                .collect();
            let padded = columns::pad_columns(&rows, 2);

            for (job, mut cells) in jobs.into_iter().zip(padded) {
                let class = StatusClass::of(job);
                let selected = global_idx == state.selected_index;
                let status_fg = Color::Indexed(status_color(class));
                let row_style = if selected {
                    Style::default()
                        .bg(Color::Indexed(colors::SELECTION))
                        .fg(Color::Indexed(colors::TEXT_ACTIVE))
                } else {
                    Style::default().fg(Color::Indexed(colors::TEXT_INACTIVE))
                };

                let activity = cells.pop().unwrap_or_default();
                let mut name = cells.pop().unwrap_or_default();
                name.push_str(columns::GAP);

                let spans = vec![
                    Span::styled(
                        format!("{} ", icons::status_icon(class)),
                        Style::default().fg(status_fg),
                    ),
                    Span::styled(name, row_style),
                    Span::styled(activity, row_style),
                    Span::styled(
                        format!(" {:>width$}", age_label(state.now, job), width = AGE_COL),
                        Style::default().fg(Color::Indexed(colors::TEXT_MUTED)),
                    ),
                ];
                lines.push(Line::from(spans));
                global_idx += 1;
            }
        }
    }
    frame.render_widget(Paragraph::new(lines), area);
}

fn draw_bottom(frame: &mut Frame, area: Rect, state: &AppState) {
    // A detail view of the selection, never a focus target: the tab titles say
    // which pane of facts is showing, and the caret says whether it is folded.
    let caret = if state.bottom_collapsed { "▸" } else { "▾" };
    let title = Line::from(vec![
        Span::styled(
            format!("{caret} "),
            Style::default().fg(Color::Indexed(colors::TEXT_MUTED)),
        ),
        tab_span("Prompt", state.bottom_tab == BottomTab::Prompt),
        Span::raw(" "),
        tab_span("Git", state.bottom_tab == BottomTab::Git),
    ]);
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::Indexed(colors::BORDER)))
        .title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if state.bottom_collapsed || inner.height == 0 {
        return;
    }

    let lines = match state.bottom_tab {
        BottomTab::Prompt => prompt_lines(state),
        BottomTab::Git => git_lines(state),
    };
    let paragraph = Paragraph::new(lines)
        .scroll((state.bottom_scroll, 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

fn tab_span(label: &'static str, active: bool) -> Span<'static> {
    if active {
        Span::styled(
            label,
            Style::default()
                .fg(Color::Indexed(colors::ACCENT))
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(label, Style::default().fg(Color::Indexed(colors::BORDER)))
    }
}

/// What the human last asked this agent — the one fact a job row cannot carry.
///
/// Never falls back to the activity summary: that is the row's own text, and
/// repeating it is what made this panel worth nothing.
fn prompt_lines(state: &AppState) -> Vec<Line<'static>> {
    let Some(prompt) = state.prompt_text() else {
        let hint = if state.selected_job().is_some() {
            "no prompt reported yet"
        } else {
            "—"
        };
        return vec![Line::from(Span::styled(
            hint,
            Style::default().fg(Color::Indexed(colors::TEXT_MUTED)),
        ))];
    };
    prompt
        .lines()
        .map(|line| {
            Line::from(Span::styled(
                line.to_string(),
                Style::default().fg(Color::Indexed(colors::TEXT_ACTIVE)),
            ))
        })
        .collect()
}

fn git_lines(state: &AppState) -> Vec<Line<'static>> {
    let git = &state.git_snapshot;
    if let Some(err) = &git.error {
        return vec![Line::from(err.clone())];
    }
    let mut lines = vec![Line::from(vec![
        Span::styled(
            truncate(&git.branch, 16).into_owned(),
            Style::default().fg(Color::Indexed(colors::BRANCH)),
        ),
        Span::styled(
            format!(" ↑{}↓{}", git.ahead, git.behind),
            Style::default().fg(Color::Indexed(colors::TEXT_MUTED)),
        ),
    ])];
    for section in [&git.staged, &git.unstaged, &git.untracked] {
        for row in section.iter().take(crate::git::LIST_CAP) {
            lines.push(Line::from(Span::styled(
                truncate(row, 36).into_owned(),
                Style::default().fg(Color::Indexed(colors::TEXT_INACTIVE)),
            )));
        }
    }
    lines
}

fn name_budget(width: usize) -> usize {
    width
        .saturating_sub(2 + 1 + columns::display_width(columns::GAP) + 1 + AGE_COL)
        .saturating_div(2)
        .max(4)
}

fn status_filter_color(filter: StatusFilter) -> u8 {
    match filter {
        StatusFilter::All => colors::ALL,
        StatusFilter::Running => colors::RUNNING,
        StatusFilter::Background => colors::MONITOR,
        StatusFilter::Waiting => colors::WAITING,
        StatusFilter::Idle => colors::IDLE,
        StatusFilter::Error => colors::PROBLEM,
    }
}

fn status_color(class: StatusClass) -> u8 {
    colors::of(class)
}
