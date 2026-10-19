# Notes

## 2026-09-21 — Windows interaction and distribution hardening

Tray/menu/toast events explicitly wake egui. Only left-button release toggles the flyout; focus dismissal waits for the new window to acquire focus. Alt+F4 hides, while the tray Quit action exits. Icon and tooltip caches are independent and only advance after successful shell updates. Copy fallbacks keep their explanation visible. Panel bounds persist, placement converts between physical pixels and egui points, and disconnected empty state no longer claims nothing is running.

The Windows CI artifact now includes `scripts/nerve.ps1` and fixtures beside both binaries. The launcher detects that bundle or accepts `-BinaryDirectory`, so installation does not require Rust. Verification refuses an existing hub, checks the surface stream count, bounds second-instance startup, and cleans up owned processes. Installer helper tests run without desktop/registry mutations. The shortcut remains a plain WScript shortcut, not an AUMID property-store registration; docs no longer claim otherwise. Native Windows tray/toast behavior still needs desktop verification; the existing monitor lookup still assumes the primary monitor origin.

<!-- mol:note:topic:hub-refresh -->
## 2026-09-18 — Panel Refresh is `POST /v1/refresh`

The surface refresh button asks the hub to reap dead local PIDs, expire pending, and republish a frame. It returns the same bare job array as `GET /v1/jobs`. A GET-only refresh only re-read RAM and could not drop ghosts. SSE is not torn down.

<!-- mol:note:topic:debug-logs -->
## 2026-09-18 — Debug logs: tracing + Unified Logging + VS Code channel

Hub and Rust surfaces use `tracing` (stderr + daily file under the OS log dir; `RUST_LOG` / `nerve-hub serve --verbose`). macOS app uses `os.Logger` (`app.nerve.Nerve`). VS Code uses a log OutputChannel named Nerve. No prompt bodies. Spawners still discard hub stdio — the hub writes its own file.

<!-- mol:note:topic:grok-loopback-ssrf -->
## 2026-09-18 — Grok `type: http` cannot reach the hub

Grok's HTTP hook runner refuses loopback and non-HTTPS URLs (SSRF). `http://127.0.0.1:17890/v1/hook` is dropped fail-open, so a plugin that only registers `type: http` never updates Nerve. Grok hooks are `command` → `hooks/grok-post.js` POSTing the same body. Hub mapping is unchanged.

<!-- mol:note:topic:notify-lease -->
## 2026-09-18 — One hub, notify lease (Tether surface removed 2026-09-29)

Every surface probes `/v1/health` before spawning; 17890 remains the lock. A second `nerve-hub serve` that loses the bind exits `AlreadyRunning`.

Notifications: the hub never talks to UserNotifications. It names SSE connections (`?surface=`), publishes `notify` on every frame (`policy` `single`|`all`, elected `owner`, `surfaces`, `watchers`), and accepts `GET`/`PUT /v1/notify`. Default `single` prefers macos → windows → vscode → tmux so two surfaces on one machine do not both banner. Missing `notify` (old hub) still means everyone fires. Presence 0→1 does not republish (the connect frame is enough); 1→2 and drops do, so the remaining owner learns.

Tether no longer hosts Nerve: `surfaces/tether` is deleted and `tether` left the preference order. The uncommitted work that made the app's views hostable (`surfaces/macos`, public `AppModel`/`JobStore`/panel inits) is in `git stash` "Tether surface work".

<!-- mol:note:topic:windows-surface -->

<!-- mol:note:topic:windows-surface -->
## 2026-09-17 — Windows: four crates, and what the move exposed

Why: "Swift runs on Windows" does not carry SwiftUI. 64% of `Nerve/` is
AppKit/SwiftUI/UserNotifications and has no Windows counterpart, so the
menu-bar app is not portable — but the architecture already was. Windows gets
its own peer surface (tray icon + flyout), not a port of anyone else's.

**Crate split.** `nerve-platform` (leaf: wire paths, pid liveness) →
`nerve-surface-core` (frames, status, grouping, hub client, ribbon weights,
Ask) → the surfaces. Two crates rather than one because `nerve-hub` must not
depend on anything called "surface-core" (invariant 7, and the release profile
says size matters) while it *does* share the `openURL` encoder with every
decoder. `nerve-tmux-surface` keeps only tmux: panes, ps, ssh, ratatui,
`summary.rs` (its `#` doubling and `#[fg=…]` markers are the tmux status-line
template language, not a general one).

**The `unsafe` exception.** `unsafe_code = "forbid"` cannot be relaxed by an
inner `#[allow]`, so `nerve-platform` opts out of `[lints] workspace = true`
and restates the lints with `deny`, carrying one audited `#[allow]` for
`OpenProcess`/`GetExitCodeProcess`. Chosen over `sysinfo` because `windows-sys`
was already in the lock file (0 new crates) and `sysinfo` would pull the whole
`windows` crate plus `objc2-core-foundation` on macOS. The root manifest points
at the exception — it is the only one.

**Four bugs the port exposed, none of them Windows-only in cause:**
- `rustix::Pid::from_raw` debug-asserts `raw >= 0` rather than answering
  `None`, so the reaper's "defensive" negative-pid arm panicked.
- `state.rs` found the breadcrumb path by searching for the literal `" · /"`,
  while the hub writes `{producer} · {project} · {host} · {cwd}`. Any non-POSIX
  cwd was invisible.
- `payload.rs` named projects with `rsplit('/')`; `build.rs` gated `openURL` on
  `starts_with('/')`.
- `slotId` called `agentPid()` twice, so the POSIX worst case was twelve `ps`
  spawns per hook event.

**Paths are classified by shape, never by host.** A macOS surface renders a
Windows producer's jobs through a tunnel (invariant 5), so `Path::is_absolute`
and `MAIN_SEPARATOR` are both wrong. `nerve_platform::path` decides
`Posix | Windows` from the string. Side effect: every case is testable on a
Mac. The POSIX output of `to_file_uri` is frozen by golden test — four
decoders read it (Rust, Swift, TypeScript, the docs).

**Windows PID: no climb.** `Get-CimInstance` costs ~500 ms to start and the
hook runs on every event. `hooks.json` uses exec form (`node` + args, no
shell), so `process.ppid` already *is* the agent host; the POSIX climb exists
only to see past a shell wrapper. A wrong-but-instant answer beats a
right-but-slow one in a fail-open hook.

**Tray icon: two design failures the spike caught, that no test would have
been written for.** A rounded cap of half the bar height turns a single
full-height band into a circle; and width taken only from each band's share of
the stack makes one running job pixel-identical to forty, because a single
status is always 100% of itself. The macOS ribbon carries count in its
*length* — `ribbon::length_factor` now does the same for the icon. Render
`examples/icon_sheet.rs` and look at it before changing this design again.

**Codex on Windows** — the documented field is `commandWindows` (JSON) /
`command_windows` (TOML), per learn.chatgpt.com/docs/hooks. Every slot in
`codex.json` now carries `py -3 …` alongside the unix `python3 …`, because
`python3` on Windows is a Microsoft Store alias stub that opens the Store
rather than running anything. A test asserts every command hook has the
override, so a new event cannot be added without one. (An earlier pass invented
`windowsCommand` and reverted it — the name matters, and a field the host
ignores means Windows users get the broken command silently.)

**Transport is a dependency now.** The hand-written HTTP/SSE reader argued in
its own doc comment that chunked size lines "can never be mistaken for an SSE
`data:` field". True, and beside the point: a chunk boundary can fall inside a
`data:` line, and the line-oriented reader would have split one frame into two
unparseable halves under load. Replaced by `reqwest` + `eventsource-stream`;
the async client rather than `reqwest::blocking` because only it exposes
`read_timeout`, which resets per read — an endless stream needs an idle
timeout, and a total budget would kill a healthy one. The runtime is
current-thread and private to a `Hub`, so every method stays blocking to its
caller and no UI loop is touched. `crates/nerve-surface-core/tests/transport.rs`
is the first test where the client and the server meet.

**Toasts are clickable after all.** The COM activator is only needed to
activate an *exited* unpackaged app; a tray surface raising a toast is by
definition running, so the WinRT `Activated` event suffices and arrives through
a safe API.

<!-- mol:note:topic:ask-graded-notify -->
## 2026-08-30 — Graded notifications: Ask gate, gentle copy

Why: every elevated Attention used to share one interrupt path; system waits and “your turn” felt the same, and chrome (status-bar yellow, “need a look”) read as alarm.

**Rule**: default interrupt channel is **Ask reasons** (`input`, `approval`, `auth`, `permission`, `decision`, `elicitation`, **`review`**) at `attention.level ≥ suggested`, on upgrade or first sight. Wait reasons paint Attention but do not banner/toast by default. Intensity: suggested soft + silent; required may sound; urgent separate toggle, calm copy. Failures / unresponsive / long success stay independent.

**Aesthetics**: calm verbs (“Your turn”, “Ready when you are”, “A review is waiting”); red only for real problems; no CRITICAL / stacked bangs; sticky clear when Ask drops; respect Pause / quiet hours / mutes.

**Surfaces**: macOS OS banners only (Settings “Your turn”). VS Code: optional Ask toast + badge Ask+problem + soft status text / a11y. tmux: `{ask}?` in default status format; `{attention}` still available. Hub never notifies.

**Supersedes**: “Needs attention covers all ≥ suggested” wording for the default interrupt path (paint rules unchanged).

<!-- mol:note:topic:vscode-surface -->
## 2026-08-24 — VS Code is a third peer surface

Why: macOS and tmux Focus often terminate in `vscode://` / `cursor://`. Once the human is in the editor, that hop is empty; the agent is already a terminal or sidebar in this window.

**Rule**: `vsc-ext/` consumes `GET /v1/stream?surface=vscode`. Native chrome only in v1 (status bar + Activity Bar tree). No OS banners (those stay macOS). No reverse-control. Hub port stays 17890; the extension may spawn `nerve-hub` the way tmux does and never kills it. Focus ranks pid → this-window terminal, then cwd → here, then folder; a foreign pid/`file://` is never compared locally. Default filter is All. Status is a third golden copy of `Subject.swift`. Build is molvis-style rslib/rspack, extension-host bundle only.

**Supersedes**: “two surfaces” wording in CLAUDE.md invariant 7.

<!-- mol:note:topic:status-palette -->
## 2026-08-24 — Status colors: five rainbow + gray

Why: waiting-as-purple asked the user to memorize a sixth “needs a look” hue. Monitor is a different job: watching a stream, not blocked and not executing.

**Rule**: shared job status is five rainbow hues (red problem, orange attention, blue running, violet monitor, green success) plus gray idle. Seven is the ceiling. Waiting on the system shares **Attention**. A background shell/subagent still executing is **Running**; only `current.type=monitor` (or active + `outcome=partial`) is **Monitor**. Ended success stays green. Chrome stays gray/slate.

**Surfaces**: `Status.painted` / tmux `colors.rs` / site `statusMeta` are the three copies of the same map. `{waiting}` remains a summary token (always empty from derivation); `{ask}` is the soft “ready for you” count in the default `@nerve_status_format`; `{attention}` still covers Wait + Ask paint rows; `{monitor}` stays in the default format.

<!-- mol:note:topic:arch-hub-topology -->
## 2026-08-23 — State hub topology (supersedes in-app ingest)

Why: surfaces must be peers; the app quitting must not take state down; tmux plugin joined as a second surface.

**Rule**: State authority is the standalone `nerve-hub` daemon (`crates/nerve-hub`, fixed `127.0.0.1:17890`, port bind = single-instance lock). Surfaces hold a `GET /v1/stream` SSE connection as their presence token; hub self-exits ~30s after the last one drops. `JobStore` in the app is a frame-fed read-only cache (writes leave only via request sinks); notifications derive from adjacent-frame diffs incl. `departed` terminal pairs, and fire from the macOS surface only. Hook wire contract unchanged; producers never spawn the hub.

**Supersedes**: in-process `IngestServer.swift` + `JobStore`-owned state semantics (deleted 2026-08-22/23, spec chain `nerve-hub` → `nerve-macos-surface-01..03` → `nerve-tmux-surface`).

## 2026-08-23 — Jobs on another machine: the ssh pane is the pane

Why: selecting the `arrhenius1` job in the tmux sidebar answered “no local pane” — the scan matched a remote job's pid and workspace against local panes, which can only fail (or, worse, collide with an unrelated local pid).

**Rule**: a surface matches a job to a pane only when the job is on **its own** machine. `machine::local_alias()` (Bonjour LocalHostName → short hostname, the hook's own order) decides; a machine that cannot name itself treats everything as local, so the fallback is the old behaviour. Same principle as the hub's reaper: “remote pids are meaningless here”.

**Rule**: a job on another machine matches the pane running **ssh to that machine** (`crate::ssh`) — `Enter` jumps to it, and the **preview never mirrors it**: that pane is a route to the *machine*, so every job on it resolves to the same pane and a mirror would show three jobs one terminal and call it each of them. Selecting a remote job announces where it runs and leaves the slot alone. Destination comes from `ps -t <pane_tty> -o args=` on panes whose `pane_current_command` is a client (`ssh`/`autossh`/`mosh`), ranked exact > first label > shared prefix, resolved through `~/.ssh/config` `HostName` so `ssh Arrhenius` matches alias `arrhenius1`. `-N` tunnels are refused: the Nerve RemoteForward is exactly that shape and has no terminal to return to. A miss now says which machine it is, not “no local pane”.

**Rule**: `Enter` on a remote job also steers the far side. One `ssh <host> sh -s` runs the *same* `pid → tty → pane` resolution over there and issues `select-window` / `select-pane`, plus `switch-client` when exactly one client is attached (with several, the one in this ssh pane cannot be told from someone else's — so none is moved). `crate::remote`, fire-and-forget on its own thread: the local jump already happened and a sidebar must never block on someone else's network. `BatchMode=yes` + `ConnectTimeout=3`, script on stdin (so the remote login shell never parses it, and the only interpolated value is a `u32` pid). Measured on Arrhenius over a live ControlMaster: 200–300ms, `pid 668866 → %25`.

**Rule**: there is no "preview" state — the highlight moves and the pane beside the sidebar follows it, local or remote. Two defects made it drift out of step and both are fixed: (a) `show()` wrote its memo *before* checking whether the window was on screen, so a selection that changed while the window was hidden (the list re-sorts every frame) counted as shown and the pane stayed on the previous job for good — the memo is now written after that check, and a hidden window is re-checked once a second rather than five times; (b) two sidebars scanning `list-panes -a` took turns yanking one agent pane out of each other's slot — a pane now carries `@nerve_preview_owner` while a sidebar shows it, and every other scan skips it.

**Rule**: it goes both ways. The highlight moves and the pane follows (`pid → tty → pane`); walk into a pane yourself and the highlight comes to you (`pane_tty → ps -t → the job whose reported pid is there`, `panes::pids_on_tty` + `state::job_running_as`, polled every 400ms and skipped while the sidebar itself has the keyboard). A pane already in the sidebar's own window is never swapped — it is visible where it is, and moving it would shuffle the layout under whoever is standing in it.

**Rule**: the same call is what makes a *remote* row follow the cursor. Selecting a remote job brings its ssh pane beside the sidebar *and* asks the far side to turn to that job, so one shared session shows the row under the cursor. Requests go through `remote::RemoteSelector`: one call in flight, newest wins, so holding `j` across three remote rows lands on the row the cursor stopped at rather than wherever the slowest call finished. `Enter` uses the same queue.

**Fix**: `Enter` no longer forgets what it showed (`PanePreview::settle`, was `restore`). Clearing the memo meant the next tick re-previewed the same job the moment the human came back to the sidebar — dragging the pane they had just walked into out of its own window.

**Why not `tmux -CC`**: control mode (what iTerm2 speaks) streams a whole window tree, cursor and resize events so a *terminal emulator* can render remote panes. This surface is a TUI inside tmux, not an emulator — it needs one imperative "select that window". Socket forwarding (`ssh -L` onto the remote tmux socket) is a dead end besides: client/server protocol versions must match (3.7c vs 3.2a here) and a forwarded socket cannot pass a tty fd.

**Fix**: the sidebar cursor is anchored to a **job id**, not a row. The list re-sorts on every frame (`compare_jobs`: attention, health, `updatedAt`), so a job arriving — or merely reporting activity — slid a different job under a cursor that never moved, and the preview swap followed it. `restore_selection(fallback)` re-finds the anchored job after every rebuild; a job that left hands the cursor to the row it held.

**Fix**: the preview memo is keyed on `(job id, pid)`. A job first seen without a pid matches on its workspace path alone — ambiguous between two agents in one project — and the frame that finally carries the pid has to re-scan instead of counting as “already showing”.

**Rule**: macOS answers the same question the same way. `RemoteMachine` (`Models/MachineConfig.swift`, next to `LocalMachine`) decides whether a job is on this Mac; `ActionService.openLocation` routes a foreign one to `openRemote`: IDE deep link → `ssh://<Host>` via NSWorkspace (Terminal by default, Host resolved from `SSHConfigWriter.loadLocalHosts()` with the same exact > label > prefix scoring) → copy the breadcrumb. A remote `file://` path is never handed to NSWorkspace. The panel button reads **Open on \<alias\>**, because opening a terminal on another machine is not the same act as revealing a folder here. `--test-swift` now also compiles `ActionService.swift` + `SSHConfigWriter.swift`.

**Layout**: the tmux surface's matching half now lives in `crate::panes` (which pane belongs to a job — decides only), `crate::preview` is what the sidebar *does* with that answer (swap / restore / jump), `crate::ssh` reads ssh panes, `crate::remote` talks to the far side, `crate::machine` answers "is this job mine". `preview.rs` had grown past the 800-line ceiling.

**Rule**: `prefix + e` on a window with no sidebar opens it **focused** — a person asking for the sidebar should land in it, not press the key twice. Auto-create stays unfocused (a new window must not steal the keyboard). `prefix + q` closes *and* selects the pane the sidebar interrupted (`@nerve_sidebar_return`), instead of letting tmux pick.

**Rule**: unit tests build `AppState` with `PanePreview::detached()`. `AppState::new` detects `$TMUX_PANE` and can swap real panes, so `cargo test` run from inside tmux used to be able to rearrange the window it was started in.

## 2026-08-23 — Sidebar detail panel is the last prompt, and it folds

Why: the panel showed the hub timeline, which `/v1/snapshot` never fills (only `/v1/events` records one), so it fell back to the row's own activity text — a second copy of what the row already said.

**Rule**: the panel under the job list is **Prompt** — what the human last asked that agent. Never falls back to the activity summary. `Space` (or a click on the title row) folds it to its title line; `Shift-Tab` switches Prompt ⇄ Git. Same fact on macOS: `Job.lastPrompt` (`Models/Subject.swift`) paints a wrapped **Prompt** block at the top of an expanded row's detail (`StatusPanelView.promptBlock`) — both surfaces read the one field, neither derives it.

**Rule**: the prompt travels as `extensions.lastPrompt` (+ `lastPromptAt`, 400 chars, `nerve_hook.py` `PROMPT_MAX`) on the `UserPromptSubmit` snapshot only — a hook is one process per event and no later run knows the prompt. The hub keeps it: `STICKY_EXTENSIONS` in `state/store.rs` carries those keys onto snapshots that omit them, the snapshot-door twin of the per-key patch merge. Live status keys (`pid`, `hookEvent`) stay non-sticky — a producer that stops reporting one means it. Still memory-only, still loopback (invariant 4).

**Rule**: a preview swap is recorded in the window option `@nerve_preview_swap` (`<foreign> <slot>`). `prefix+q` and the next sidebar start both undo a stranded one — a sidebar killed rather than closed never runs its own restore, and the moved pane would otherwise stay in the wrong window.

**Fix**: `auto-close` treated a pane with no `@nerve_pane_role` as "nothing left" and killed windows that still held live shells. Only the sidebar ever sets a role, so every pane must *be* the sidebar before the window closes (`only_sidebar_remains`).

## 2026-08-23 — tmux sidebar: cursor stays in the job list; pane match is identity-first

Why: the cursor wandered into the Activity panel (nothing to select there), and selecting a job never swapped the right-hand pane.

**Rule**: the sidebar has exactly one cursor and it lives in the job list. The bottom panel is a view *of* the selection — `Ctrl-d`/`Ctrl-u`/`PgDn`/`PgUp`/wheel scroll it, `Shift-Tab` switches Prompt ⇄ Git, and nothing moves the cursor there (`Focus` enum deleted). `j`/`k` clamp at the ends; `h`/`l`/`Tab` always cycle the filter.

**Rule**: job → pane matching is identity first. `extensions.pid` → `ps -p … -o tty=` → the pane on that tty wins outright (`PaneRank` compares `tty` before `path`/session/window); a scan row that matches nothing is skipped, never fatal. Before: one `?` on a non-matching pane aborted the whole `list-panes -a` scan, so the preview never found anything, and a same-window content pane sharing the project dir outranked the agent's real pane.

**Rule**: preview swaps are coalesced — `sync_preview` runs once per loop tick, not per keypress, and `restore()` is the only thing that clears `showing()` (an unswap mid-`show` used to re-fire a swap every 200ms).

## 2026-07-23 — Focus A+B (no reverse-control)

- Product: **do not** C/D (approve / submit_input / in-panel chat). Enhance **A+B**.
- A Attention semantics already clear (input vs approval vs failure; bg wait ≠ Attention).
- B Focus first-class:
  - Hook `location.openURL`: Cursor/VS Code deep link when host is IDE, else `file://` workspace URI; `focusHint` = producer · project · host · path.
  - Panel primary action **Open / Focus** (borderedProminent), then Copy. No Approve.
  - Notification click → `focusAndOpenJob` (select + expand + open location) + best-effort panel reveal.
  - Honest copy: “Your turn in agent” / “Approval needed in agent” — return to agent UI, never “Type here”.
- Local open/copy actions stay re-usable (not stuck `.succeeded`).

## 2026-07-22 — Notifications: suggested attention was silent

- Bug: `NotificationService` only fired for `attention.level` `required` / `urgent`.
- Hooks set Stop / idle_prompt to **`suggested`** (paints Status.attention) — no banner.
- Fix: escalate notify on `suggested` + `required` (Settings “Needs attention”); `urgent` separate.
- Also fire Failures on `attention.reason=failure` (tool/turn fail without outcome).
- Auth denied → log to Console; empty body no longer drops banner.

## 2026-07-22 — Ribbon ambient motion (settings)

- Menu-bar ribbon keeps transition ease (length / segment cross-fade).
- Continuous styles via `RibbonMotionStyle`: transitionsOnly | breathe | shimmer | statusPulse | full.
- Settings → General → Ribbon motion; gated by `animationsEnabled` + Reduce Motion.
- Live paint path is SwiftUI `MenuBarRibbonLabel` (`TimelineView` ~24fps when ambient active).

## 2026-07-22 — Remote tunnels: local SSH config + known_hosts

- Machines list = Hosts from `~/.ssh/config` (refresh only; no manual add).
- Managed block: **RemoteForward + ExitOnForwardFailure** only.
- Connect: `ssh -O forward` if ControlMaster up; else `ssh -n -N`. OTP → Terminal first.

## 2026-07-22 — Shell / monitor bg wait ≠ Attention

- Toast / `background_tasks` for shell · subagent · “shell/monitor still running” → **Running** (never Attention).
- Monitor-**only** queue (or pure monitor toast) → `current.type=monitor` + `outcome=partial` → **Success green** (phase done, waiting for feedback).
- Mixed shell+monitor → Running. Real human idle remains Stop empty bg / idle_prompt without bg toast.

## 2026-07-22 — SessionStart is starting/Ready, not Running

- `/new` / SessionStart with no user prompt yet must **not** paint Running.
- Hook: SessionStart → `lifecycle=active` + `current.type=starting` + summary “Ready” (no attention).
- `starting` ≠ `idle`: idle + attention.input is your_turn (Stop); starting is open with no turn yet.
- App: `starting` / `idle` / `booting` → Inactive; only thinking/tool/subagent/info → Running.
- First UserPromptSubmit flips to thinking → Running.

## 2026-07-22 — Session leave paths (P0/P1)

- Authoritative: SessionEnd (`endReason` + outcome success|cancelled) → evict.
- Supersede: same UI `slot` + new SessionStart → previous ended (`superseded`).
- Local PID reaping: snapshot `extensions.pid` + hub's 5s maintenance tick; dead local process → `process_gone`.
- No manual Dismiss control — leave paths above only (Copy remains as local action).
- Snapshots carry `extensions.slot` + `extensions.pid` when known.

## 2026-07-22 — Public docs on the website only

- Removed repo `docs/` (former `IMPLEMENTATION.md`).
- Product handbook is `index/src/docs/content.ts` with SPA routes under `/docs/*`.
- Root / plugin / sources READMEs are short pointers to the site.
- Hook model: **one job per conversation**; subagents only refine main `current`; ignore tool noise when `agent_id` is set (except permission / lifecycle brackets).

## 2026-08-23 — Performance pass + oversized-file split

- **Hub and tmux surface stay Rust-only.** Wiring, the sidebar TUI, and the
  hub daemon have no bash/Python on the hot path. `surfaces/tmux/nerve.tmux`
  is a fail-open trampoline that `exec`s `nerve-tmux-surface install`;
  `nerve.conf` is gone. TPM still needs a `*.tmux` file.
- **Hub runtime** — `#[tokio::main(flavor = "current_thread")]`. A handful of
  loopback connections and a 150 ms pump do not need a worker pool.
- **Hub frame render** — `sse::Frame` borrows the store and serialises a
  `JobsSeq` straight into a sized buffer. It used to collect a `Vec<JobView>`
  (and before that, two `serde_json::Value` trees) just to walk them again; a
  frame is the hub's hottest work (full job set per 150 ms window).
  `jobs_json()` stays for `GET /v1/jobs` and the state tests.
- **tmux sidebar tick** — `refresh_from_hub` re-grouped and re-sorted the whole
  list 5×/s whether or not a frame arrived. It now compares snapshot `Arc`
  identity and rebuilds only on arrival; `now`, pane-follow and git keep their
  own throttles. Ratatui draws on input, a new snapshot, or once a second
  (relative ages) — not five times a second on a quiet pane.
- **tmux subprocess tax** — one `ps -ax` snapshot per 250 ms answers pid→tty,
  tty→pids and ssh destinations (was one spawn per pid / per ssh pane). Git
  panel is one `status --porcelain=v1 -b` (was five git processes). `~/.ssh/config`
  is `OnceLock`. Opening the sidebar no longer runs `sh -lc` (login shell).
- **Status derivation** — `StatusClass::of` no longer lower-cases `reason` /
  `current.type` per job (it is asked once per job for the filter bar, once per
  section and once per row). Vocabulary matching is `eq_ignore_ascii_case`.
  Row text (`activity_text`, `cell_text`, `truncate`) borrows instead of copying.
- **`percent_decode` was wrong, not just slow** — it pushed each *byte* as a
  `char`, so `/Users/me/项目` decoded to mojibake and the Git panel got a path
  that does not exist. Decodes into bytes now, `from_utf8_lossy` at the end.
- **Hooks are host-native, not rust-only.** Official type + official language:
  Claude Code `command` exec form (`node` + `args` → `hooks/nerve.js`);
  Codex `command` (`python3 ${PLUGIN_ROOT}/hooks/nerve.py`, as their docs show);
  Grok `type: "http"` (Grok POSTs the event; hub `/v1/hook` maps it because
  there is no user script). Mapping for Claude/Codex is in the native script
  and POSTs `/v1/snapshot`.
- **macOS store** — `conversationJobs` / `openJobs` are derived once per write
  (`rederive()`), pre-sorted. Eight readers used to filter the dictionary and
  re-sort; a filter keeps order, so every section now inherits the sort.
- **File split** (user rule: 800 lines max) — `StatusPanelView` 1331 → 550,
  `MenuBarRibbonController` 1391 → 640, `SettingsView` 964 → 782, cut on the
  files' own `// MARK:` seams into 9 new files. Types that moved out of the
  file that declared them lost `private`.
- **`scripts/nerve.sh`** — the EXIT traps read locals of the function that
  armed them, so every verify run ended `hub_pid: unbound variable` *after*
  printing ALL OK. Trap state is script-scope now.
- **Site** — deleted 6 modules unreachable from any route (`Sources`, `Signal`,
  `Quiet`, `HomePage`, `usePointer`, `useScrollProgress`) — leftovers of the
  `index-page/` → `index/` move.
  - **Left alone, deliberately:** ~400 lines of `.sources` / `.signal` CSS in
    `global.css` and the `config.ts` data they read. The macOS page groups
    those selectors with live ones (`.product-page .story, .product-page
    .signal, …`), so untangling needs a visual check, and the config data is
    product copy, not dead machinery.

## 2026-08-23 — Site styling unified on Tailwind v4

Tailwind v4 was installed and `@import`ed but did nothing: 26 components carried
**zero** utility classes and 5327 lines of hand-written CSS did all the work,
against a `:root` token set that ran parallel to an unused `@theme`.

- **One token source.** `@theme` in `styles/theme.css` is it — colours, fonts,
  `--container-page`, shadows, easing, and the two breakpoints (`lap` 900px,
  `wide` 960px) that are not on Tailwind's scale. The parallel `:root` block and
  every `var(--ink)`-style alias are gone.
- **Components carry their own layout.** Nav, Footer, LanguageToggle, the four
  hub sections, Stage/Story/Get, all four docs views and the tmux guide are
  utilities now. Shared recipes (`homeLink`, `sectionTitle`, `focus-ring`) live
  in `lib/ui.ts` and one `@utility`.
- **What stays CSS, on purpose:** the two device mockups
  (`styles/mac-preview.css`, `styles/tmux-preview.css`). A laptop lid, a
  drifting wallpaper and pulsing status dots are pseudo-elements and keyframes;
  no utility expresses those. They now own their sizes as variants
  (`--hero` / `--card`) instead of being reached into by parent selectors.
- **Layering matters.** Un-layered CSS beats every Tailwind layer, so the
  remaining rules sit in `@layer components` — otherwise a utility on the
  element loses to a stylesheet that merely loads later.

Net: 5327 → 1356 CSS lines; the built stylesheet 84.7 kB → 65.6 kB
(18.5 → 13.6 kB gzip), total bundle roughly flat.

**How it was kept honest.** A Playwright script shot 12 routes × 4 widths
before and after every step and a Pillow diff compared them; the pass ends at
**0.000% visibly-changed pixels** across all 48. Two things only that harness
caught: a `<p>` added to the hero for a `heroLede` that never existed (dead CSS
with no markup behind it), and `list-disc` on the docs lists, which preflight
had been stripping all along.

Three `App.test.tsx` assertions keyed on class names (`.home-hero-tagline`,
`a.docs-card`, `is-active`); they assert structure and `aria-current` now — the
nav marks its active link for assistive tech either way, which it did not before.


## 2026-09-22 — Separate turn completion from human attention

Supersedes the earlier Stop/idle_prompt → Ask and Wait → orange rules. A normal Stop with no structured background tasks reports `current.type=completed`, `attention.level=none`, lifecycle active, no session outcome. Surfaces paint green and macOS says “Turn complete”; only SessionEnd removes the row. Explicit input/approval/review still requests attention. An idle_prompt without structured background work is ignored, preserving completion or a pending Ask; PostCompact stays thinking. Background shell/subagent/monitor rules remain unchanged.

System waits derive Waiting (neutral gray), not human Attention (orange). Resumed execution suppresses stale attention paint; terminal success outranks old attention. macOS adds persistent status words, a Needs you / Active / Turn complete priority grouping, visible grouping menu, connection/empty-state guidance, and accurate local-action labels and results. Custom palettes are preserved; the former default palette migrates.

## 2026-09-24 — Three-way hook mapper parity, and the fixture corpus that holds it

Why: `nerve.js` `mapEvent`, `nerve.py` `_map_event` and hub `map::map_event` had drifted. `permissiondenied` painted Ask on Grok only — Claude/Codex dropped the event and left the row on `completed` while the human was blocked on a denial. `stopcancelled` was not even in the Python allow-set. `postcompact` said "Compacting context" on the hub and "Context compacted — continuing" in both hooks; `posttoolusefailure` titled `"Tool failed"` on the hub and `"{tool} failed"` in both hooks; Python's posttooluse subagent set was missing `get_command_or_subagent_output`; the supersede ghost said "Session replaced" on Python only. `crates/nerve-hub/tests/golden_parity.rs` was named for this and never compared a mapper.

**Rule**: `fixtures/hook-events/` is the contract. Each case is a `{name}.json` input plus a `{name}.facets.json` of **normalized** facets — every key present, absent → `null` (`ended` → `false`), `current.startedAt` stripped so timestamps cannot flake. `crates/nerve-hub/tests/hook_map_parity.rs` runs all three mappers against every fixture and diffs them at each other and at the golden. `nerve.js --map <fixture>` / `nerve.py --map <fixture>` are the adapters — argv, not env, not `NERVE_*`, always exit 0.

**Rule**: contested semantics now have one answer, majority wins. `permissiondenied` ≡ `permissionrequest` (waiting / required / approval / "Approval needed in agent"). `stopcancelled` ≡ `elicitation` ≡ `yourTurn`. `postcompact` empty bg → `"Context compacted — continuing"` (hub stopped reusing `precompact`). `posttoolusefailure` title → `"{tool} failed"` (`Facets.attention_title` became `Option<String>`); `stopfailure` stays `"Turn failed"`. Supersede ghost → `"Session ended (superseded)"`. `current.startedAt` is present on every non-ended job (was three silent schemas). Extensions drop `""` as well as `null` (Python was leaking `hookEvent: ""`). Compact JSON in `permission`'s dumped `tool_input` (`separators=(",", ":")`) — Python's default `", "`/`": "` was the fourth divergence and the fixture corpus caught it as a red `permission-request`.

**Rule**: eight events now map on all three — `cwdchanged` → info "Workspace changed"; `elicitation` → yourTurn; `elicitationresult` → info from structured `summary`/`title`; `taskcreated`/`taskcompleted` → info with `current.name` from `title`/`name`; `teammateidle` → info (a teammate idling is not a human Ask). Eight stay **silent by name** (`setup`, `userpromptexpansion`, `posttoolbatch`, `messagedisplay`, `instructionsloaded`, `configchange`, `directoryadded`, `filechanged`): `SILENT_NOOP` / `_SILENT_NOOP` is an explicit list so "never paint a facet" is testable, not implied by a catch-all. They return before alias / slot / `ps` / HTTP / disk.

**Rule**: manifests register what the mappers handle. `hooks.json` gained `StopCancelled`; `codex.json` gained `PermissionDenied`, `PostToolUseFailure`, `StopFailure`, `StopCancelled`, `TaskCreated`, `TaskCompleted`, `TeammateIdle`, `CwdChanged`, `Elicitation`, `ElicitationResult`; `grok.json` gained the task/teammate/cwd/elicitation six. `test_hook_slots.py` pins each host's set — over-registration is a no-op, under-registration is how `StopCancelled` never reached Node.

**Rule**: the job body is now uniform across all three producers. `actions` is location-aware (Open when `openURL` exists, Focus when only `focusHint`, always Copy, plus `open_logs` whenever `location.logPath` is set — macOS `ActionService` already implemented the reveal and was waiting on the declaration). `current.startedAt` is present on every non-ended job. Extensions drop `""` as well as `null`. Hub `/v1/hook` now publishes `extensions.slot` and `extensions.pid`; `grok-post.js` computes both with the same algorithm as `nerve.js` and injects them, so vsc/tmux focus works on the Grok path too. Background summaries list every task (each truncated to 40 chars, tail to 120) and `current.name` is a compact kind label — `monitor` / `shell` / `subagent` / `mixed`.

**Rule**: the hook hot path pays each `scutil` / `ps` climb once per UI slot, not once per event. An identity cache (`$TMPDIR/nerve-hook/identity-<terminal token>.json`, 24 h TTL) holds `{alias, pid}` keyed by the terminal token, which is free to compute; the full `slotId` hash folds the alias in and cannot be the key. SessionStart invalidates it so a renamed machine or restarted host is picked up. `stateDir()` mkdirs once per process. The synchronous `appendFileSync` to `nerve-hook.log` on the POST success path is gone — nothing read it. A SessionStart that supersedes a ghost now sends **one** envelope with `jobs: [ghost, new]`; the envelope was always plural and `apply_snapshot` already takes a `Vec`.

**Rule**: `crates/nerve-surface-core/src/frame.rs` `JobExtensions` gained `model`, `agent_type`, `slot` (additive; unknown keys were already ignored). The hub kept them in its open map the whole time — the drop happened at surface decode, which is why no surface could show a model name.

**Rule**: `index/src/docs/content.ts` is the only public handbook (invariant 8). The PostCompact-no-bg row said `idle · attention.reason=input` / Attention and was wrong — it is `thinking · "Context compacted — continuing"` / Running. Both the EN and ZH blocks are corrected, the eight new event rows are added, `open_logs` is documented beside Open/Focus/Copy, the extension keys and the empty-string filter are listed, and `/docs/status` now carries the reason-specific status words.
