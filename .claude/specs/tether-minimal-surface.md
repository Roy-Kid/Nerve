# Tether minimal surface

Status: plugin and tab indicators implemented · 2026-10-19

## Product decision

Nerve remains a display-only, protocol-v2 surface. The hub owns jobs, lifecycle,
ordering, classification, machine state, summaries and notification policy.
The Tether plugin owns search, filtering, disclosure, focus and layout only.

The user's visual requirement is binding: use one identical solid circle for
every status and distinguish it through color. No visible status words, alternate
status glyphs, badges, counters, attention cards or explanatory section headings.
Task names, machine names and expanded task content remain readable data.
Names and explanations for controls belong in tooltips and accessibility labels.

## This delivery

- Keep the existing compact 400×450 host popover and work at 280px width upward.
- Use a quiet header, brand, connection circle, circle-only status filters and a
  search icon. All is a neutral circle; the seven status filters use the canonical
  Nerve palette/order. Waiting and idle retain the shared canonical gray; idle is
  subdued. The pressed filter has a neutral background and inset selection ring,
  distinct from hover. Clicking the active filter resets to All.
- Rows show one status circle and a task name. Use a neutral disclosure surface,
  not a second status word or different glyph. Expand by click or Enter/Space.
- Expanded content shows authoritative activity, producer, location, update time
  and the hub's existing timeline, when present. Do not manufacture absent data
  or add a persistent transcript store. Content remains selectable for copying.
- Search is revealed by the search icon or Cmd/Ctrl+F. Search name, machine,
  producer, location and authoritative activity, case-insensitively. Preserve
  received order; intersect search and status filter. When delivered to the page,
  Escape clears the search,
  then collapses it, then collapses the focused row. Do not intercept the final
  Escape, so the host can close its popover. ArrowUp/Down/Home/End navigate rows.
- Group by the existing display.machineGroup in received order. Show a machine
  header only when useful (multiple machines or an explicitly offline machine).
  Include configured remote machines without jobs; use frame.machines state and
  display.sourceOnline, never local liveness guesses. Offline jobs remain visible
  with a subdued presentation and an accessible stale/offline description.
- Connecting, disconnected, permissions denied, missing host bridge and protocol
  upgrade use the same circle with a tooltip/accessibility explanation. A retry
  icon is available on recoverable failures. Empty/no-match states use a circle
  with explanatory tooltip/accessibility text, without visible prose.
- Keep the last valid full snapshot on disconnect, mark it stale, replace it on
  reconnect, accept a lower revision only after hub identity changes, and never
  let a malformed frame advance the revision or erase the last valid snapshot.
- Close the previous stream before manual retry; cancel timers, unregister event
  delivery and close the stream on pagehide. Panel and background remain separate
  sessions; only background reports the existing summary ribbon to host.status.
- Keep light/dark appearance, reduced-motion support, native focus and selectable
  content. Do not add dependencies. The status ribbon's descriptive label remains
  its existing host tooltip/accessibility payload; it is not panel status copy.

## Reuse decisions

- EXTEND surfaces/tether/web/model.js: canonical palette, authoritative display,
  activity and summary-based host status already exist. Add presentation search
  here; TypeScript editor helpers cannot be imported into this standalone package.
- REUSE native HTML details/summary, search input, Intl.DateTimeFormat and DOM
  focus/selection. No custom disclosure or listbox framework.
- REUSE hub JobView timeline and display projections; no hub change is needed.
- REUSE the existing WebKit integration harness and DemoStream. Update its stale
  PluginHost initializer to the current public consent/streaming API.

## Explicit integration follow-up (not claimed as shipped here)

1. Session focus remains a future Web-plugin capability. Per-tab status is now
   integrated as the generic tabIndicator contribution described below. Never
   inject input or match a terminal by free-text task title.
2. Targeted notifications require a background instance identity, notify capability,
   hub client preferences registration, connection-target matching and result
   reporting. host.notify exists, but the Web gateway only supports exact-URL SSE,
   not the HTTP operations needed for registration. Extend the host capability
   contract first; retain hub notification election/deduplication and do not infer
   notifications from status transitions in this plugin.
3. Host theme synchronization requires theme tokens/change events in the plugin
   environment. Until then follow OS appearance; do not claim custom theme parity.
4. The current host popover intercepts Escape before WebKit and may close the
   popover immediately. Page-level Escape handling works when the host delivers
   that key; changing the host's popover key routing is a separate host change.

## Acceptance

- All status controls/indicators have identical circular geometry; status words
  are absent from the default visible panel, available via title/ARIA instead.
- Every canonical status can be filtered, including Monitor, Completed and Idle.
- Search, filter and disclosure work together without changing hub job order.
- Detail/focus survive snapshots for surviving rows. Removed jobs disappear and
  move keyboard focus sensibly; updates do not repeatedly replace the focused DOM.
- A disconnect retains and marks existing rows; permission/protocol errors explain
  recovery accessibly; malformed/stale frames cannot replace good data.
- Background summary ribbon and panel both run from installed package bytes;
  closing/disabling sessions cancels transports without stopping the hub.
- Run Node model tests and real Tether WebKit integration tests. Inspect light and
  dark rendering, narrow width, populated/expanded/search/empty/disconnected states
  in one batch, fix findings, and perform at most one confirmation batch.

## Verification record

- `node --test testing/tether/model.test.cjs`: 5 passed. Covers authoritative
  classification/order, every status filter, query intersections and invalid data.
- `swift test --package-path testing/tether`: 2 passed, live-hub test skipped.
  Real installed bytes run in Tether WebKit; covers panel/background, status-only
  symbols, search, timeline, keyboard movement, stable disclosure/focus, stale and
  invalid snapshots, hub restart, empty offline machines, grant denial, manual
  stream replacement and cancellation on disable. Background WebKit readiness
  waits for a jobs summary (up to 5 seconds), rather than its initial offline report.
- Live hub was not running on loopback, so no live-hub verification is claimed.
- `cd index && npm test && npm run build`: 27 tests passed; static build passed.
- Impeccable mechanical scan of `surfaces/tether/web`: no findings.
- Browser fixture inspection: light/dark, 400px and 280px, collapsed/expanded,
  search/no-match and disconnected states. Fixed retry-button overflow; the single
  confirmation pass reported no horizontal overflow at 280px with retry visible,
  8 circle-only filters and no status-word elements. Native WebKit snapshot also
  captured successfully. The temporary static server was stopped afterward.
- Existing unrelated working-tree changes were preserved. No dependencies added;
  no hub or sibling Tether implementation changed.

## 2026-10-19 — Tab indicator extension

The user requested that task state appear before the tab name and that this slot
become part of the plugin system. This delivery changes both repositories.

- TetherPluginKit adds data-only PluginTerminalIdentity and PluginTabIndicator,
  plus TetherPlugin.tabIndicators. PluginRegistry resolves enabled providers in
  registration order, without knowing any producer or business status names.
- Web manifests declare tabIndicator. host.tabIndicators atomically replaces a
  session's set (max 512, unique targets, 24-bit colors, bounded labels); undeclared
  callers and malformed payloads are rejected. Background activation also works
  for a plugin declaring only tabIndicator. Unload clears that session's set.
- Tether's existing terminalName/ShellTTY discovery supplies the controlling TTY;
  ProcessInfo.hostName supplies the local machine name and the authenticated remote
  lease executes hostname once per shell connection. Unknown identities yield no
  mark. Host/TTY matching is exact; renaming a tab does not change it. Hostnames
  must identify machines uniquely; saved-host aliases are deliberately not guessed.
- Horizontal and vertical tab bars draw the same 6pt circle, with producer-supplied
  color and tooltip/ARIA. A split workspace shows its focused terminal's mark.
  A disconnected/ended shell has no plugin identity and uses existing host chrome.
- Hooks report only raw terminalHost/terminalName, obtained from the hook process's
  inherited controlling TTY and OS hostname. This remains valid with piped stdin
  and does not depend on agent PID discovery. Failure to read a TTY remains fail-open. Grok
  reuses the Node forwarder; Codex uses Python's standard library.
- Hub preserves this identity in extensions.terminal and emits frame.tabIndicators:
  first task per terminal in authoritative job order; offline origins are omitted.
  The localOnly stream recomputes this projection after filtering remote jobs.
- Nerve's background page translates hub status to its existing canonical palette
  and forwards the full set. Disconnect/protocol error clears marks. It never
  classifies task text or resolves competing task priority itself.
- Existing instances need updated hook bytes and a subsequent event, not new shells.
  The host build and installed web package both need updating for this capability.

Verification:

- Hub: `cargo test -p nerve-hub`, 182 passed. Includes raw hook identity propagation,
  hub selection/order, offline exclusion, and the additive frame contract key.
- Web host: 42 passed. Includes manifest gating, malformed/duplicate/color rejection,
  session-specific cleanup and a tabIndicator-only plugin with no bottom ribbon.
- Plugin kit: 10 passed, including exact identity and enabled-provider lookup.
- App: 333 tests passed in a clean temporary build directory; after adding terminal
  identity and native rendering tests, both new targeted tests passed. The default
  app scratch directory had a pre-existing missing-artifacts error, so verification
  used `/tmp/tether-tab-indicator-build` without altering that cache.
- Nerve WebKit integration: 2 passed; live hub test skipped. Real package bytes
  report the canonical orange mark from the background page only, clear it when
  disconnected and exercise full panel/stream lifecycle behavior.
- Node: 4 hook tests and 6 model tests; Python: 4 hook tests. The macOS hook test
  runs inside a real PTY and asserts hostname/TTY metadata with redirected stdin.
- Native horizontal/vertical screenshots inspected: a 6pt circle precedes the tab
  name in both layouts. No count, status word, alternate status glyph or animation.
- Remote hostname lookup compiles and reuses the authenticated lease; no live SSH
  server was used. Existing ambiguous remote TTY discovery remains fail-closed.
