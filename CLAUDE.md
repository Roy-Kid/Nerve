# CLAUDE.md

<!-- nerve:harness:managed begin -->
<!-- Thin agent router. Public product docs live on the website, not under docs/. -->

## What this repo is

**Nerve** — agent/job status for your machines. Hooks push jobs over loopback HTTP to the **`nerve-hub` daemon** (`127.0.0.1:17890`, state authority); peer **surfaces** render it: the macOS menu-bar app, the tmux plugin, the VS Code extension, and the Windows tray. Nerve does **not** run agents.

Stack: `nerve-hub` + tmux helper (Rust, `crates/`), SwiftUI menu-bar app (`Nerve/`), tmux plugin entry (`surfaces/tmux/`), marketplace hooks (`plugins/nerve/` — Claude Node, Codex Python, Grok HTTP), Rsbuild React site + handbook (`index/`).

## Where things live

| Zone | Path |
|------|------|
| State hub daemon | `crates/nerve-hub/` (ingest contract, SSE frames, refcount lifecycle) |
| macOS app (surface) | `Nerve/Nerve/` (App, Models, Store, Services incl. `Services/Hub/`, UI) |
| tmux surface | `surfaces/tmux/` (TPM entry) + `crates/nerve-tmux-surface/` (helper) |
| Windows native UI | `surfaces/windows/` (C# / WinUI 3 window, list, settings and native tray) |
| Windows presentation core | `crates/nerve-windows-surface/` → `nerve-windows-core` (Rust binary, NDJSON protocol; legacy egui is opt-in only) |
| Shared surface logic | `crates/nerve-surface-core/` (frames, status, grouping, hub client) |
| OS primitives | `crates/nerve-platform/` (wire paths, process liveness) |
| VS Code surface | `vsc-ext/` (Activity Bar + status bar; rslib/rspack) |
| Tether surface | `surfaces/tether/` (Swift package `NervePlugin`; compile into Tether) |
| Marketplace plugin | `plugins/nerve/` — Claude `hooks/nerve.js`, Codex `hooks/nerve.py`, Grok `hooks/grok-post.js` (command POST to `/v1/hook`) |
| Grok HTTP mapper | `crates/nerve-hub/src/hook/` — `POST /v1/hook` |
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
pwsh scripts/nerve.ps1 -Help                        # Windows launcher (install / verify / run)
./scripts/nerve.sh --test-swift                     # Swift value-type unit harness
cd vsc-ext && npm test                              # extension host unit tests
cd index && npm test && npm run build               # site tests + static build
```

## Invariants (do not break casually)

1. **One job per conversation** — id `{producer}:{session_id}`. Subagents refine main `current` only; no child session rows. Batch/chain producers may use `role=group|member` with tree panel (display).
2. **Fail-open hooks** — exit 0 always; never block the agent. No `NERVE_*` env; ingest fixed `http://127.0.0.1:17890`, served by `nerve-hub` (the port bind is its single-instance lock). Producers never spawn the hub.
3. **Status from structured fields only** — never free-text message classification (`notification_type`, `background_tasks`, event name).
4. **Memory-only runtime jobs** — jobs/timelines/pending live in `nerve-hub` process RAM; hub exits ~30s after the last surface disconnects (SSE refcount). Settings + managed `~/.ssh/config` only on disk.
5. **Open alias ingest** — any snapshot `alias` shows; Settings Machines are tunnels only (Hosts from local `~/.ssh/config` + known_hosts; managed block is RemoteForward-only; ControlMaster via `ssh -O forward` when master is up). Tunnels target 17890, so remote producers *and* remote surfaces reach the hub for free.
6. **Display only** — Nerve never reverse-controls agents or jobs (no approve/cancel/submit_input). Local actions: **Open/Focus** (location) + Copy; rows leave via SessionEnd / slot supersede / PID reap. Attention means “return to agent UI”, not “type here”.
7. **Surfaces are peers** — macOS app, tmux plugin, VS Code extension, Windows tray, and Tether plugin only consume the hub contract (`GET /v1/jobs`, `GET /v1/stream` full frames with `departed` terminal states); none owns state, none knows the others. The hub never sends OS notifications. It *does* count open streams and publish a **notify lease** (`notify.policy` `single`|`all`, elected `owner`) so two surfaces on one machine do not both banner the same Ask. Default is `single` (prefer `macos`, then `tether`, then `windows`, then `vscode`, then `tmux`). `PUT /v1/notify` sets the policy. Each surface still dedupes locally on the Ask channel (`reason ∈ Ask`, `level ≥ suggested`, first sight or level upgrade, 120 s window) and only fires when the lease says it may. Hook wire contract is unchanged.
8. **Public docs** — edit `index/src/docs/content.ts` (and site UI), not a repo `docs/` folder. Keep root/plugin READMEs as short pointers.
9. **Native application UI, Rust binary core** — macOS uses SwiftUI/AppKit and Windows uses WinUI 3/Windows Shell. Native hosts render and perform platform interaction; core state/protocol/presentation rules live in Rust binaries. Do not introduce a hand-drawn cross-platform GUI or move domain rules into C#. VS Code and tmux use their host's own UI facilities. Windows host/core protocol is in `surfaces/windows/README.md`.

## Default workflow

1. Product copy / API handbook → `index/src/docs/content.ts` (+ pages under `src/pages/`)
2. Hook lifecycle → `plugins/nerve/hooks/nerve.js` (Claude) · `nerve.py` (Codex) · hub `/v1/hook` (Grok HTTP)
3. State semantics / ingest contract → `crates/nerve-hub/` (golden parity tests guard it)
4. App UI / surface glue → `Nerve/Nerve/`; tmux surface → `surfaces/tmux/` + `crates/nerve-tmux-surface/`; VS Code surface → `vsc-ext/`; Windows tray → `crates/nerve-windows-surface/`; Tether plugin → `surfaces/tether/`
5. Capture decisions → `.claude/notes/notes.md`

<!-- nerve:harness:managed end -->
