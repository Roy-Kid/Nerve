# CLAUDE.md

<!-- nerve:harness:managed begin -->
<!-- Thin agent router. Public product docs live on the website, not under docs/. -->

## What this repo is

**Nerve** — agent/job status for your machines. Hooks push jobs over loopback HTTP to the **`nerve-hub` daemon** (`127.0.0.1:17890`, state authority); peer **surfaces** render it: the macOS menu-bar app, the tmux plugin, the VS Code extension, and the Windows tray. Nerve does **not** run agents.

Stack: `nerve-hub` + tmux helper (Rust, `crates/`), SwiftUI menu-bar app (`Nerve/`), tmux plugin entry (`surfaces/tmux/`), marketplace hooks (`plugins/nerve/` — Claude Node, Codex Python, Grok HTTP), Rsbuild React site + handbook (`index/`).

## Where things live

| Zone | Path |
|------|------|
| State hub daemon | `crates/nerve-hub/` (ingest contract, SSE frames, resident lifecycle) |
| macOS app (surface) | `Nerve/Nerve/` (App, Models, Store, Services incl. `Services/Hub/`, UI) |
| macOS UI package | `surfaces/macos/` (used by the menu-bar app) |
| Tether surface | `surfaces/tether/` (installable plugin) |
| Tether tests | `testing/tether/` (model and host integration) |
| tmux surface | `surfaces/tmux/` (TPM entry) + `crates/nerve-tmux-surface/` (helper) |
| Windows surface | `crates/nerve-windows-surface/` (tray icon + flyout) |
| Shared surface logic | `crates/nerve-surface-core/` (protocol decoding, display helpers, hub client) |
| OS primitives | `crates/nerve-platform/` (wire paths, process liveness) |
| VS Code surface | `vsc-ext/` (Activity Bar + status bar; rslib/rspack) |
| Marketplace plugin | `plugins/nerve/` — Claude `hooks/nerve.js`, Codex `hooks/nerve.py`, Grok `hooks/grok-post.js` (command POST to `/v1/hook`) |
| All-producer event mapper | `crates/nerve-hub/src/hook/` — `POST /v1/hook` |
| Website + **public docs** | `index/` → routes `/docs/*`; body `index/src/docs/content.ts` |
| Demo / scripts | `fixtures/`, `scripts/` |
| Passive agent notes | `.claude/notes/` |
| Active specs (if any) | `.claude/specs/` |

**There is no `docs/` tree.** Product handbook is the site:

- `/` — home hub (macOS, tmux, VS Code sections)
- `/docs` · `/docs/get-started` · `/docs/plugin` · `/docs/machines`
- `/docs/status` · `/docs/ingest` · `/docs/tmux` · `/docs/vscode` · `/docs/windows` · `/docs/privacy`

```bash
cd index && npm run dev   # http://localhost:3000/docs
```

## Commands

```bash
./scripts/nerve.sh --help                           # dev launcher (explicit flags only)
./scripts/nerve.sh --run                            # build + open Nerve.app
./scripts/nerve.sh --build --tmux --tmux-reload     # install tmux surface + reload
./scripts/nerve.sh --demo                           # POST demo fixture (hub must be up)
cargo test --workspace                              # hub (incl. hook mapper) + tmux-surface tests
./scripts/nerve.sh --verify-loop                    # ingest contract E2E (needs hub)
./scripts/nerve.sh --verify-surface                 # macOS surface regression
./scripts/nerve.sh --verify-tmux                    # tmux surface E2E (isolated tmux)
./scripts/nerve.sh --verify-vscode                  # VS Code surface unit tests
./scripts/nerve.sh --check-windows                  # compile portable crates for Windows (no linker)
./scripts/nerve.ps1 -Help                           # Windows launcher (install / verify / run)
./scripts/nerve.sh --test-swift                     # Swift value-type unit harness
cd vsc-ext && npm test                              # extension host unit tests
cd index && npm test && npm run build               # site tests + static build
node scripts/capture-vscode-surface.mjs             # capture the VS Code surface screenshot
```

## Invariants (do not break casually)

1. **One job per conversation** — id `{producer}:{session_id}`. Subagents refine main `current` only; no child session rows. Batch/chain producers may use `role=group|member` with tree panel (display).
2. **Fail-open hooks** — exit 0 always; never block the agent. No `NERVE_*` env; ingest fixed `http://127.0.0.1:17890`, served by `nerve-hub` (the port bind is its single-instance lock). Producers never spawn the hub.
3. **Status from structured fields only** — never free-text message classification (`notification_type`, `background_tasks`, event name).
4. **Memory-only runtime jobs** — jobs and timelines live in hub RAM. `nerve-hub ensure` owns discovery, detached startup and readiness; the fixed loopback bind is the singleton lock. Closing surfaces never stops the daemon. Machine identity and machine configuration are persistent, tasks are not.
5. **One authority per machine** — hooks POST raw events to `/v1/hook?producer=…` on their own machine. The local hub alone manages SSH configuration, LocalForward tunnels and origin-scoped remote full snapshots. Replicate `/v1/stream?localOnly=true` only; never relay aggregated tasks. Remote disconnect retains its last snapshot with an offline machine state; only its origin hub reaps its PIDs. Clear deletes local jobs only.
6. **Display only** — Nerve never runs, approves, cancels or submits input to agents. The hub publishes Open/Focus, Copy and log actions. Surfaces execute navigation and OS notifications, not task control.
7. **Surfaces consume protocol v2** — `/v1/frame` and SSE publish `protocolVersion`, hub identity, monotonic revision, jobs, departed, authoritative display/order/summary, machine states and connection-targeted notifications. `/v1/jobs` uses the same projection. No surface lifecycle, classification, business deduplication, notification election or SSH execution. UI search/filter/selection/layout may only change presentation. Hub notification routing uses instance + connection identity, Ask escalation, 120-second deduplication and single/all policy (macOS, Windows, VS Code, tmux priority). Send at most once; never replay on reconnect. Old protocols require an upgrade.
8. **Public docs** — edit `index/src/docs/content.ts` (and site UI), not a repo `docs/` folder. Keep root/plugin READMEs as short pointers.

## Default workflow

1. Product copy / API handbook → `index/src/docs/content.ts` (+ pages under `src/pages/`)
2. Hook lifecycle → `plugins/nerve/hooks/nerve.js` (Claude) · `nerve.py` (Codex) · hub `/v1/hook` for every producer
3. State semantics / ingest contract → `crates/nerve-hub/` (hub contract and lifecycle tests guard it)
4. App UI / surface glue → `Nerve/Nerve/`; tmux surface → `surfaces/tmux/` + `crates/nerve-tmux-surface/`; VS Code surface → `vsc-ext/`; Windows tray → `crates/nerve-windows-surface/`
5. Capture decisions → `.claude/notes/notes.md`

<!-- nerve:harness:managed end -->
