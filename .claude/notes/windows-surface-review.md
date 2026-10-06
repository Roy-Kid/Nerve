# Windows surface review — 2026-09-22

Scope: Windows surface source and tests, startup/installer integration, shared
hub/store contract, and comparison with the macOS StatusPanelView. This is not
a claim that every macOS, website, extension, or hook source file was audited.
Existing uncommitted changes were retained.

## Findings addressed

- **P1: Hidden-window event delivery.** Tray/menu work was consumed from the
  egui update callback. Added a winit adapter that services it on a bounded
  250 ms heartbeat and immediate repaint wakeups, without relying solely on
  Windows painting a hidden HWND. The SSE reader remains independent.
- **P1: Menu event identity.** Removed the unused direct muda 0.18 dependency.
  All menus and event handlers use tray-icon's re-export of muda 0.17. Separate
  versions own separate global event channels.
- **P1: Double-click toggles closed.** Windows emits Down, Up, DoubleClick, Up.
  A stateful decoder consumes the trailing release. Explicit Open resets focus
  acknowledgement, and the frame issuing Focus ignores its old focus input.
- **P1: Repeat launch does nothing.** Manual launches now activate the owner
  through the existing loopback lock. The listener is nonblocking, accepts at
  most 16 requests per tick, and exposes no job-control commands. Autostart
  remains hidden and does not activate another instance.
- **P1: Footer controls unavailable.** Settings used a non-clickable label and
  was omitted on empty snapshots. It is now a real button in a fixed footer.
  At minimum height, egui's default minimum scroll height also covered Quit;
  the body now respects the remaining space.
- **P2: Multi-monitor placement.** Use actual monitor origins and destination
  scale instead of assuming (0,0). Position commands are converted through the
  current viewport's scale, including moves between different DPI monitors.
- **P2: Visual consistency.** Centralized body/metadata sizes, 28pt controls,
  neutral surfaces and spacing; loaded Segoe UI with CJK fallback; aligned
  titles and activity text left; added stable job widget IDs and expanded-row
  treatment. Tray and panel follow their separate Windows theme settings.
- **P2: Repeated work.** Throttled registry, icon, and notification derivation
  to four times a second; hidden frames skip panel layout. Initial tray
  registration includes an icon. Preference-write failures are logged.

## Validation

- `cargo test -p nerve-windows-surface -p nerve-surface-core -p nerve-platform -p nerve-hub --quiet`: passed.
- Final Windows suite: 138 tests passed; native desktop test separately passed.
- `cargo clippy -p nerve-windows-surface --all-targets -- -D warnings`: passed.
- `cargo test -p nerve-windows-surface --lib hidden_window_double_click_activation_and_quit -- --ignored --nocapture`: passed.
  Uses a real winit/wgpu window, injected tray/menu event queues, and a real
  loopback activation connection. Preferences use a temporary file; no hub is
  started or modified. Renderer screenshot: `target/windows-panel-smoke.png`.
- Minimum 320×240 panel: Settings, notification checkbox and Quit tested with
  zero and 100 jobs using actual egui pointer input.
- Website: 27 tests and production build passed. Used Node with the installed
  npm-cli.js directly because the system npm wrapper points at a missing file.
- Release binary built at `target/release/nerve-windows-surface.exe`.

## Desktop acceptance still required

The native harness does not physically click Explorer's notification area.
Its environment prints `Error removing system tray icon` from tray-icon during
cleanup, although the window lifecycle assertions pass. Real Explorer menu
delivery, icon removal, Explorer restart, and mixed-DPI hardware behavior need
desktop acceptance. Windows styling is closer to the macOS panel's hierarchy
and spacing, not a pixel-identical SwiftUI/material implementation. No running
installation was replaced and no changes were committed.

## Follow-up: console allocation and panel redesign

The first pass missed the console executable started by DetachedSpawner:
redirecting stdin/stdout/stderr alone still lets Windows allocate a console.
The Windows spawn path now uses CREATE_NO_WINDOW. A real PowerShell child
calls GetConsoleWindow and exits unsuccessfully if a console exists; it passed.
The new release PE header was also checked: subsystem 2 (Windows GUI).

Replaced the oversized in-panel ribbon and footer buttons with a compact
macOS-style header, vector icon controls, settings page with switches, rounded
surface and subtle shadow. Defaults are 340×360, matching macOS. Existing user
sizes remain preserved. Light, dark, settings and empty-state screenshots were
rendered and inspected; they are target/windows-panel-{smoke,dark,settings,empty}.png.
Windows/shared-core tests, the native lifecycle harness, and Clippy passed.

The old release executable was locked by PID 8076. Built with an alternate
filename and delivered target/release/nerve-windows-surface-refresh.exe. Restarted
only the verified old surface into the new executable; the existing hub PID
39244 was left running to preserve runtime jobs. Existing console windows
belonging to that previously launched hub are not retroactively removed by the
new process creation flag.
After the old process exited, the canonical target/release/nerve-windows-surface.exe was replaced with the same new binary; both files have matching SHA-256 hashes.

## Follow-up: panel interaction parity — 2026-10-06

- Added Up/Down selection and expansion in the rendered section order, with
  Enter/Space toggling details and automatic scrolling to the selected row.
  Selection is keyed by job ID and stale selection/expansion is removed.
  Mouse selection can continue with the keyboard; focused header controls keep
  their Enter activation. Settings does not consume job navigation.
- Added a visible bottom-right resize grip using Windows native southeast
  resizing. The existing viewport bounds and size persistence still apply.
  Reserved space beneath the list keeps the grip clear with many jobs.
- Updated the English and Chinese Windows handbook instructions.
- Validation: Windows surface suite (144 tests) passed; 7 panel interaction
  tests cover navigation, boundaries, empty jobs, scrolling, control focus,
  mouse-to-keyboard handoff, and resize commands in both list and settings.
  Portable-crate all-feature tests passed before these panel-only changes.
  Portable-crate all-target/all-feature Clippy, formatting, and diff checks passed.
  Native lifecycle test passed and light screenshot was inspected; this run did
  not emit the earlier tray removal error. Site: 27 tests and build passed.
  Release executable rebuilt at target/release/nerve-windows-surface.exe.
- No running installation was replaced. Physical Explorer tray/menu delivery,
  Explorer restart, actual resize dragging, and mixed-DPI hardware acceptance
  remain unverified. This adds panel interaction parity, not every macOS feature.

## Follow-up: dragging and idle visibility — 2026-10-06

User reported an immovable flyout and an apparently empty tray icon. The
borderless window had a resize grip but no move region. Added a native drag
target in the header's blank area, excluding the settings and quit controls.
The live hub returned no jobs; the previous idle icon was a left-aligned,
55%-alpha gray dash. It now draws a centered, opaque, thicker neutral dash
with explicit contrast for light and dark taskbars. This addresses idle
visibility, not proof of every possible Explorer blank-icon failure.

Validation: 146 Windows surface tests, Clippy and native lifecycle test passed.
Tests cover drag commands in list/settings and no overlap with header buttons,
plus idle opacity, contrast and centering at every DPI rung. Actual mouse
dragging and Explorer rendering remain outside the injected-input harness.
WinUI 3 with a C# Windows surface and the existing Rust hub was recommended
for maintainability; migration itself has not been implemented.

Release rustc with thin LTO crashed (access violation/stack buffer overrun)
when building this follow-up, including with a short alternate output name.
Built a development GUI-subsystem binary at target/debug/nerve-fix.exe instead.
Replaced only the verified running checkout surface PID 43348 with repair
surface PID 20864; installed hub PID 10376 was retained. This is a development
validation binary, not a verified release artifact or installed upgrade.

## Native UI / Rust binary boundary — 2026-10-06

User explicitly requested native frameworks for application UI and Rust binaries
for core logic. Windows now defaults to `surfaces/windows` (C# / WinUI 3) plus
`nerve-windows-core` (Rust). The library package keeps its existing name so core
regression tests and references remain stable. Legacy egui files are preserved
behind `legacy-ui`, excluded from default builds and Windows release packaging.
The default dependency tree contains no egui, eframe, winit or wgpu. macOS
continues using its existing SwiftUI/AppKit UI; this change migrates Windows.

The host uses native title bar/borders for movement and resizing, ListView for
focus/navigation/scrolling, WinUI settings controls, native Shell tray via the
.NET NotifyIcon wrapper, and Windows App SDK notifications. Rust outputs v1
presentation frames over private stdin/stdout NDJSON: ordered rows, counts,
derived status/color, tooltip/bitmap, preferences and notification events. It
retains hub subscription, Ask lease/dedupe and Open/Copy rules. EOF stops the
core; hiding the window does not. Native host owns the existing activation
port 17891 before starting a core. Fixture mode never attaches to the live hub
or persists preferences; smoke tests use an ephemeral activation port.

Windows launcher, installer and CI now publish/install the complete
self-contained x64 WinUI package and both Rust binaries. Payload tests cover
native DLL/resources, language subfolders and incomplete runtime downloads.
Handbook and repository routing instructions reflect this boundary.

Validation: named portable-crate Rust suites and default/all-feature Clippy
passed; host protocol tests passed (DTO version/bitmap validation, real Rust
fixture projection, unsupported-command rejection, grouping, stdin EOF exit).
Rust hub/core Release and native WinUI Release publish succeeded. Native fixture
smoke exited 0, rendered 2 jobs and a native title bar, and captured light,
detail, settings (including scrolled footer), dark and empty states; screenshots
were visually inspected. Website: 27 tests and production build passed.

Replaced only checkout repair surface PID 20864 with native Release host
PID 45632 and Rust core PID 15140. Existing installed hub PID 10376 retained;
read-only health confirms one Windows stream. Installed application payload and
Start Menu shortcut were not replaced. A combined second-launch verification
command was rejected by automatic approval review with only `blocked by policy`;
read-only process/health checks succeeded. Live duplicate-launch acceptance,
Explorer restart, notification delivery and physical mixed-DPI drag/resize
remain unverified. No changes committed or published.
