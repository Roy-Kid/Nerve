# nerve — tmux surface

A tmux **sidebar** aligned with [tmux-agent-sidebar](https://hiroppy.github.io/tmux-agent-sidebar/):
filter bar, job list (grouped by host), and a foldable Prompt/Git panel — fed by the
same `nerve-hub` the menu-bar app reads (`http://127.0.0.1:17890`).

## Docs

→ **Website** `/docs/tmux` — from repo root:

```bash
cd index && npm run dev
```

## Install

```bash
cargo install --path crates/nerve-tmux-surface
```

```bash
# ~/.tmux.conf — trampoline finds the helper and runs `nerve-tmux-surface install`
run-shell ~/path/to/nerve/surfaces/tmux/nerve.tmux

# or, if the helper is already on PATH:
run-shell "nerve-tmux-surface install"
```

## Keys (inside tmux)

| Key | Action |
|-----|--------|
| `prefix + e` | Sidebar cycle (see below) |
| `j` / `k` | Move selection; right pane previews the job |
| Click | Select row; double-click jumps |
| Wheel | Over the list: move selection; over the panel: scroll it |
| `Enter` | Jump to matched tmux session/window/pane; if none, open IDE deep link (`cursor://` / `vscode://`) — never a folder |
| `h` / `l` / `Tab` | Cycle status filter |
| `Shift+Tab` | Prompt ⇄ Git |
| `Ctrl-d` / `Ctrl-u`, `PgDn` / `PgUp` | Scroll the bottom panel — the selection never leaves the job list |
| `Space`, or click the panel title | Fold the panel to its title line and give the rows to the list |

### `prefix + e` and `prefix + q`

| Key | Action |
|-----|--------|
| `prefix + e` (closed) | Open sidebar, focused |
| `prefix + e` (open, unfocused) | Focus sidebar |
| `prefix + e` (open, focused) | Unfocus back to content pane |
| `prefix + q` (sidebar focused) | Close sidebar |
| `q` / `Esc` in sidebar | Unfocus (same as `prefix+e` while focused) |

Typical flow: `prefix+e` opens beside you → `prefix+e` again to operate the
list → `Enter` to jump → `prefix+e` or `q` to return to the agent pane →
`prefix+q` to dismiss the sidebar.

> Matches common tmux plugin habits: one key toggles **focus**, another **closes**
> the split. Opening without stealing focus keeps you in the agent terminal.

## Options

| Option | Default |
|--------|---------|
| `@nerve_sidebar_width` | `15%` |
| `@nerve_sidebar_position` | `left` |
| `@nerve_sidebar_bottom_height` | `20` |
| `@nerve_sidebar_auto_create` | `on` |
| `@nerve_sidebar_key` | `e` |
| `@nerve_sidebar_close_key` | `q` |

## Verify

```bash
./scripts/nerve.sh --verify-tmux
```

Display only — Enter jumps to the agent pane; no approve/cancel. Notifications stay on the macOS app.
