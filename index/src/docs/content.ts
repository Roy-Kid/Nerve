/** Structured docs content for website subpages (source of truth; repo READMEs point here). */

import type { Locale } from '../i18n/messages';

export type DocNavItem = {
  slug: string;
  title: string;
  summary: string;
};

export const docNav: DocNavItem[] = [
  {
    slug: 'get-started',
    title: 'Get started',
    summary: 'Run Nerve, use the ribbon, verify with demo jobs.',
  },
  {
    slug: 'plugin',
    title: 'Agent plugins',
    summary: 'Claude Code, Codex, Grok marketplace hooks → local ingest.',
  },
  {
    slug: 'machines',
    title: 'Machines & remotes',
    summary: 'SSH reverse tunnels so remote agents report to this Mac.',
  },
  {
    slug: 'tmux',
    title: 'tmux plugin',
    summary: 'Install, keys, options, remotes, and troubleshooting.',
  },
  {
    slug: 'vscode',
    title: 'VS Code extension',
    summary: 'Status bar and Activity Bar job list inside the editor.',
  },
  {
    slug: 'tether',
    title: 'Tether plugin',
    summary: 'Jobs inside Tether, sharing one hub with the menu bar.',
  },
  {
    slug: 'windows',
    title: 'Windows tray',
    summary: 'Notification-area icon, flyout panel, toasts, and install.',
  },
  {
    slug: 'status',
    title: 'Status & lifecycle',
    summary: 'One job per session, facets, ribbon colors, design scope.',
  },
  {
    slug: 'ingest',
    title: 'Ingest API',
    summary: 'Loopback HTTP for snapshots, events, live status updates, and actions.',
  },
  {
    slug: 'privacy',
    title: 'Privacy',
    summary: 'Memory-only runtime, loopback only, fail-open hooks.',
  },
];

const zhDocNav: DocNavItem[] = [
  {
    slug: 'get-started',
    title: '开始使用',
    summary: '运行 Nerve、使用状态条，并用演示任务验证。',
  },
  {
    slug: 'plugin',
    title: 'Agent 插件',
    summary: 'Claude Code、Codex、Grok 市场钩子 → 本地接入。',
  },
  {
    slug: 'machines',
    title: '机器与远程主机',
    summary: '通过 SSH 反向隧道，让远程 agent 向这台 Mac 报告。',
  },
  {
    slug: 'tmux',
    title: 'tmux 插件',
    summary: '安装、按键、选项、远程主机与故障排查。',
  },
  {
    slug: 'vscode',
    title: 'VS Code 扩展',
    summary: '编辑器里的状态栏和 Activity Bar 任务列表。',
  },
  {
    slug: 'tether',
    title: 'Tether 插件',
    summary: '在 Tether 里看任务，和菜单栏共用一个 hub。',
  },
  {
    slug: 'windows',
    title: 'Windows 托盘',
    summary: '通知区图标、弹出面板、系统通知与安装。',
  },
  {
    slug: 'status',
    title: '状态与生命周期',
    summary: '每个会话一个任务、状态切面、状态条颜色与设计边界。',
  },
  {
    slug: 'ingest',
    title: '接入 API',
    summary: '用于快照、事件、实时状态更新与操作的本地回环 HTTP。',
  },
  {
    slug: 'privacy',
    title: '隐私',
    summary: '运行时仅存内存、只用本地回环、钩子失败开放。',
  },
];

export type DocBlock =
  | { type: 'p'; text: string }
  | { type: 'ul'; items: string[] }
  | { type: 'ol'; items: string[] }
  | { type: 'code'; lang?: string; code: string }
  | { type: 'table'; headers: string[]; rows: string[][] }
  | { type: 'callout'; title?: string; text: string }
  | { type: 'h2'; text: string }
  | { type: 'h3'; text: string };

export type DocPage = {
  slug: string;
  title: string;
  lede: string;
  blocks: DocBlock[];
};

export const docPages: DocPage[] = [
  {
    slug: 'get-started',
    title: 'Get started',
    lede: 'Nerve aggregates the status your agents already push, over an open loopback protocol you can implement yourself.',
    blocks: [
      { type: 'h2', text: 'Requirements' },
      {
        type: 'p',
        text: 'The hub and the hook plugins run anywhere. Which surface you get depends on the machine.',
      },
      {
        type: 'table',
        headers: ['Machine', 'Surface', 'Needs'],
        rows: [
          ['macOS 14+', 'Menu-bar ribbon', 'Xcode 15+ to build the app from source'],
          ['Windows 10/11', 'Tray icon + flyout', 'Nothing beyond the binary'],
          ['Linux, or any of them', 'tmux sidebar · VS Code extension', 'tmux 3.x · VS Code 1.90+'],
        ],
      },
      {
        type: 'ul',
        items: [
          'Rust 1.82+ (to build nerve-hub from source)',
          'Node 18+ (Claude hook plugin) or Python 3 (Codex hook plugin)',
          'OpenSSH client (for remote machines)',
        ],
      },
      { type: 'h2', text: 'Run from the repo' },
      {
        type: 'code',
        lang: 'bash',
        code: `./scripts/nerve.sh --run

# optional: demo jobs (hub must already be running)
./scripts/nerve.sh --demo

# optional smoke checks
./scripts/nerve.sh --verify-loop`,
      },
      {
        type: 'p',
        text: 'On macOS a continuous ribbon appears in the menu bar — no Dock icon, no floating window. On Windows the same state folds into a notification-area icon, with a compact status panel behind it.',
      },
      { type: 'h2', text: 'Ribbon gestures' },
      {
        type: 'table',
        headers: ['Gesture', 'Action'],
        rows: [
          ['Left-click', 'Status panel (↑/↓, Enter expand; expanded rows show the last prompt, detail, timeline, actions)'],
          ['Right-click', 'Settings… or Quit Nerve'],
        ],
      },
      {
        type: 'p',
        text: 'Settings: General (menu bar, panel, endpoint) · Machines (SSH tunnels) · Appearance (ribbon size & colors) · Notifications · About.',
      },
      { type: 'h2', text: 'Website (this site)' },
      {
        type: 'code',
        lang: 'bash',
        code: `cd index
npm install
npm run dev      # local preview
npm run build    # static → index/dist/`,
      },
    ],
  },
  {
    slug: 'plugin',
    title: 'Agent plugins',
    lede: 'One marketplace plugin reports session lifecycle to Nerve as jobs. Each host uses its official hook type. Fail-open, no project environment variables.',
    blocks: [
      {
        type: 'callout',
        title: 'One job per conversation',
        text: 'Job id is {producer}:{session_id} (never append agent_id). Subagents are not separate panel rows — they only refine the main session’s current facet. Tool noise inside subagents (agent_id set) is ignored. The app also drops legacy child rows (parentJobId / role=subagent / multi-segment ids). Stop waits for input; SessionEnd removes the row. If the host skips SessionEnd on /new · /clear · fork, the next SessionStart for the same UI slot posts an ended snapshot for the previous session_id. Local snapshots carry pid so a closed terminal can be reaped automatically.',
      },
      { type: 'h2', text: 'Install' },
      {
        type: 'table',
        headers: ['Harness', 'Install', 'Producer id'],
        rows: [
          [
            'Claude Code',
            '/plugin marketplace add Roy-Kid/nerve → /plugin install nerve@nerve',
            'claude-code',
          ],
          [
            'Codex',
            'codex plugin marketplace add Roy-Kid/nerve → codex plugin add nerve@nerve',
            'codex',
          ],
          ['Grok', 'Same Claude-compatible marketplace / plugin', 'grok'],
        ],
      },
      { type: 'h3', text: 'Claude Code' },
      {
        type: 'code',
        lang: 'text',
        code: `/plugin marketplace add Roy-Kid/nerve
/plugin install nerve@nerve`,
      },
      { type: 'h3', text: 'Codex CLI' },
      {
        type: 'code',
        lang: 'bash',
        code: `codex plugin marketplace add Roy-Kid/nerve
codex plugin add nerve@nerve

# local development
codex plugin marketplace add /ABS/PATH/TO/nerve
codex plugin add nerve@nerve`,
      },
      {
        type: 'p',
        text: 'Interactive Codex: /plugins → nerve → install. Trust hooks with /hooks if prompted.',
      },
      { type: 'h2', text: 'Official hook types' },
      {
        type: 'table',
        headers: ['Host', 'Official type', 'What runs'],
        rows: [
          ['Claude Code', 'command (exec form)', 'Official: `"command": "node", "args": ["${CLAUDE_PLUGIN_ROOT}/hooks/nerve.js"]`. Native Node, no shell.'],
          ['Grok', 'command', '`node grok-post.js` POSTs the event JSON to http://127.0.0.1:17890/v1/hook. Grok `type: http` refuses loopback (SSRF); connection failure is fail-open.'],
          ['Codex', 'command', 'Official docs: python3 ${PLUGIN_ROOT}/hooks/…. Codex has no HTTP hook type. Script: hooks/nerve.py, async.'],
        ],
      },
      {
        type: 'p',
        text: 'Claude maps in Node and POSTs /v1/snapshot. Codex maps in Python and POSTs /v1/snapshot. Grok’s command hook POSTs the raw event to /v1/hook and the hub maps that body. Every path fail-opens if the hub is down. Every official lifecycle event is registered as a slot (Claude omits WorktreeCreate/Remove: a no-op there fails worktree creation). Events without a facet map fire and no-op rather than invent status.',
      },
      { type: 'h2', text: 'What hooks report' },
      {
        type: 'table',
        headers: ['Hook', 'Main session facets', 'Ribbon'],
        rows: [
          ['SessionStart', 'active + starting (“Ready”)', 'Inactive'],
          ['UserPromptSubmit', 'active + thinking', 'Running'],
          ['PreToolUse / PostToolUse (main thread)', 'active + tool / subagent', 'Running'],
          ['SubagentStart', 'active + current.type=subagent', 'Running'],
          ['SubagentStop (no other bg work)', 'active + thinking', 'Running'],
          ['Stop + shell/subagent bg tasks', 'current.type=subagent', 'Running'],
          ['Stop + monitor-only bg tasks', 'current.type=monitor + outcome=partial', 'Monitor'],
          ['Stop empty bg / idle_prompt', 'attention.reason=input', 'Attention'],
          ['idle_prompt + shell/agent toast', 'current.type=subagent', 'Running'],
          ['idle_prompt + monitor toast', 'current.type=monitor + partial', 'Monitor'],
          ['PermissionRequest / permission_prompt', 'attention.reason=approval', 'Attention'],
          ['PreCompact', 'thinking (or remaining bg tasks)', 'Running / Monitor'],
          ['PostCompact + bg tasks', 'subagent · monitor', 'Running / Monitor'],
          ['PostCompact no bg', 'idle · attention.reason=input', 'Attention'],
          ['SessionEnd', 'ended + endReason (+ success|cancelled)', 'Leaves panel'],
          ['SessionStart (new id, same UI slot)', 'previous id → ended (superseded)', 'Old row leaves'],
          ['Other Notification (no type)', 'active + info', 'Running'],
        ],
      },
      {
        type: 'ul',
        items: [
          'Fail-open: if Nerve is down, Claude/Grok command hooks are a non-blocking connection failure, and nerve-hub hook exits 0. Never block the agent.',
          'No environment variables. Ingest URL is fixed: http://127.0.0.1:17890/v1/hook.',
          'Slot memory lives in the hub (producer + workspace), not a temp file, so /new without SessionEnd still closes the previous session_id.',
          'UserPromptSubmit also sends extensions.lastPrompt (+ lastPromptAt), trimmed to 400 characters. Only that one hook run sees the prompt, so the hub carries it forward onto later snapshots — every surface shows it: the tmux sidebar’s Prompt panel, the Prompt block in an expanded macOS row, and the VS Code tree tooltip.',
          'Local actions: Open / Focus (location) first, then Copy. Rows leave via SessionEnd, slot supersede, or PID reap — no Dismiss / Approve / submit_input.',
          'location.openURL + focusHint on every snapshot so the panel can jump back to the agent UI.',
          'Alias = free-form machine label (prefer Bonjour LocalHostName on macOS).',
          'Status is never inferred from free-text — only event name + structured fields.',
        ],
      },
      { type: 'h2', text: 'Development' },
      {
        type: 'code',
        lang: 'bash',
        code: `python3 sources/agents/tests/test_nerve_hook.py
node --test plugins/nerve/hooks/nerve.test.js
cargo test -p nerve-hub hook`,
      },
      {
        type: 'p',
        text: 'Prefer marketplace install over the legacy sources/agents/codex/hooks.json template.',
      },
    ],
  },
  {
    slug: 'machines',
    title: 'Machines & remotes',
    lede: 'Settings → Machines manages SSH reverse tunnels so a remote’s loopback ingest reaches the Mac running Nerve.',
    blocks: [
      {
        type: 'p',
        text: 'Machines in Settings are for tunnels only. Any free-form alias in a snapshot is shown in the panel — there is no allow-list.',
      },
      { type: 'h2', text: 'On the Mac running Nerve' },
      {
        type: 'ol',
        items: [
          'Open Settings → Machines',
          'This Mac is always present (alias = hostname short name / Bonjour LocalHostName)',
          'Remote list is loaded from your ~/.ssh/config — use the refresh control to re-read Hosts',
          'Enable a Host, then Connect. Nerve only injects RemoteForward; connection details stay in your SSH config',
        ],
      },
      {
        type: 'callout',
        title: 'known_hosts & OTP / captcha clusters',
        text: 'Host keys come from local known_hosts. For MFA/captcha: run `ssh <alias>` in Terminal first so ControlMaster is up; Connect uses `ssh -O forward` on that master when available.',
      },
      {
        type: 'callout',
        title: 'Alias tip',
        text: 'Use the remote hostname short name as the alias so hooks match without extra setup. Campus DHCP hostnames often differ from Bonjour LocalHostName — mismatch can drop snapshots.',
      },
      { type: 'h2', text: 'On the remote' },
      {
        type: 'ol',
        items: [
          'Install the nerve marketplace plugin',
          'Run sessions as usual',
          'Hooks POST to http://127.0.0.1:17890 — the tunnel forwards that to the host Nerve',
        ],
      },
      { type: 'h2', text: 'A remote job in the panel' },
      {
        type: 'p',
        text: 'A job reported by another machine keeps that machine’s alias, and its workspace path belongs to that machine — so nothing here tries to open it locally. The panel’s primary action says where it goes (“Open on arrhenius1”) and opens an ssh session to that machine instead, through whatever you have registered for ssh: URLs (Terminal by default). The Host comes from your own ~/.ssh/config, matched against the alias by name, so ssh Arrhenius reaches a machine that calls itself arrhenius1. IDE deep links (cursor://, vscode://) are still opened as they are — those route to their own remote. With no matching Host, Nerve copies the location rather than pretend it opened something.',
      },
      {
        type: 'p',
        text: 'The tmux sidebar answers the same question its own way: Enter on a remote job jumps to the pane you are already ssh’d into that machine from. Neither surface ever matches a remote pid or a remote path against something local.',
      },
    ],
  },
  {
    slug: 'tmux',
    title: 'tmux plugin',
    lede: 'The menu bar and tmux read the same live updates, so both stay current without depending on each other.',
    blocks: [
      {
        type: 'callout',
        title: 'Quick setup',
        text: 'From a repo checkout, one command builds the helper, wires the tmux entry point, and reloads tmux.',
      },
      {
        type: 'code',
        lang: 'bash',
        code: `./scripts/nerve.sh --build --tmux --tmux-reload`,
      },
      {
        type: 'p',
        text: 'nerve-tmux-surface opens a sidebar pane (aligned with tmux-agent-sidebar): a status filter bar, scrollable job list, and a foldable Prompt/Git panel — Prompt shows what you last asked that agent, which is the one thing a status row cannot tell you. prefix + e toggles it. Data comes from nerve-hub over SSE — the same frames the menu-bar app reads. No menu bar required, so this works on a headless Linux box too.',
      },
      { type: 'h2', text: 'Install the helper' },
      {
        type: 'code',
        lang: 'bash',
        code: `cargo install --path crates/nerve-tmux-surface`,
      },
      {
        type: 'p',
        text: 'That puts nerve-tmux-surface in ~/.cargo/bin. Working from a checkout you do not have to install at all — the plugin looks in the repo’s own target/release first, then /opt/homebrew/bin, /usr/local/bin, ~/.cargo/bin, and finally PATH. The helper is the whole plugin: install (keys, options, hooks), the sidebar TUI, and the toggle/close commands. nerve.tmux is a fail-open trampoline that finds that binary and runs install — there is no bash left in the wiring.',
      },
      { type: 'h2', text: 'Wire it into tmux' },
      {
        type: 'code',
        lang: 'bash',
        code: `# ~/.tmux.conf — from a checkout
run-shell ~/src/nerve/surfaces/tmux/nerve.tmux

# ~/.tmux.conf — with TPM handling the clone and updates
set -g @plugin 'Roy-Kid/nerve'
run-shell ~/.tmux/plugins/nerve/surfaces/tmux/nerve.tmux`,
      },
      {
        type: 'callout',
        title: 'Why TPM needs the second line',
        text: 'TPM auto-sources only the *.tmux files at the root of a plugin repo. Nerve is a monorepo and the entry point lives at surfaces/tmux/nerve.tmux, so @plugin does the cloning and updating while run-shell points at the entry point. The run-shell line alone is enough if you would rather clone it yourself.',
      },
      {
        type: 'p',
        text: 'Sourcing it more than once is safe: keybindings are rebound, and a second sidebar toggle is a no-op when one already exists in the window.',
      },
      { type: 'h2', text: 'Keys' },
      {
        type: 'table',
        headers: ['Key', 'Action'],
        rows: [
          ['prefix + e', 'Sidebar cycle: open (focused) → unfocus → focus. Auto-created sidebars open unfocused, so a new window leaves you typing where you were'],
          ['j / k', 'Move the selection — the pane beside the sidebar follows it, on this machine or another'],
          ['(walk into a pane)', 'The reverse also holds: focus a pane yourself and the highlight comes to the job running there'],
          ['Enter', 'Jump to that job’s real session/window/pane — or, for a job on another machine, the pane you are ssh’d into it from, with that machine’s tmux selecting the job’s own window'],
          ['h / l / Tab', 'Cycle status filter'],
          ['Shift+Tab', 'Prompt ⇄ Git bottom panel'],
          ['Ctrl-d / Ctrl-u, PgDn / PgUp', 'Scroll the bottom panel — the selection never leaves the job list'],
          ['Space', 'Fold the panel to its title line (click the title does the same)'],
        ],
      },
      { type: 'h2', text: 'Options' },
      {
        type: 'table',
        headers: ['Option', 'Default', 'What it sets'],
        rows: [
          ['@nerve_sidebar_width', '15%', 'Sidebar width (columns or %)'],
          ['@nerve_sidebar_position', 'left', 'left or right'],
          ['@nerve_sidebar_bottom_height', '20', 'Bottom panel height (0 hides)'],
          ['@nerve_sidebar_auto_create', 'on', 'Auto-create sidebar on new windows'],
          ['@nerve_sidebar_key', 'e', 'Toggle key after prefix'],
          ['@nerve_sidebar_close_key', 'q', 'Close key after prefix'],
        ],
      },
      {
        type: 'callout',
        title: 'Display only',
        text: 'Enter jumps to the matched tmux session/window/pane for that job. There is no approve, cancel, or submit — and tmux never opens a Finder folder.',
      },
      { type: 'h2', text: 'How it relates to the hub' },
      {
        type: 'ul',
        items: [
          'The helper is a consumer, not an authority. It looks for nerve-hub (/opt/homebrew/bin → /usr/local/bin → ~/.cargo/bin → PATH) and starts one if nothing answers 127.0.0.1:17890, at most once every 10s.',
          'The sidebar pane holds exactly one GET /v1/stream?surface=tmux. That open connection is this surface’s hub refcount.',
          'One sidebar per window (auto-created by default). Kill the pane and the hub loses a subscriber.',
          'Nothing is written to disk: no launchd job, no systemd unit, no state file. What state there is lives in tmux options, which die with the tmux server.',
        ],
      },
      { type: 'h2', text: 'Remote hosts get this for free' },
      {
        type: 'p',
        text: 'If a machine already has a RemoteForward tunnel back to your Mac (see Machines & remotes), then 127.0.0.1:17890 on that remote already is your Mac’s hub. Install the helper there, add the run-shell line, and the remote’s tmux shows your jobs. The helper has no concept of “remote” to configure — no host, no port, no env var. It is the same design that lets agent hooks on a remote POST to loopback and mean this Mac.',
      },
      {
        type: 'p',
        text: 'A job reported by another machine has no pane on this one, so it is matched by the session you are watching it through: the pane running ssh to that host. Enter jumps there — the honest local answer to “take me back to that agent”. The host is matched against the machine alias the producer reported, through your ~/.ssh/config when the Host you typed is spelled differently (ssh Arrhenius → arrhenius1).',
      },
      {
        type: 'p',
        text: 'That pane is a route to the machine, not to the job: every job on that machine resolves to the same ssh session. So moving the highlight onto a remote job does two things at once — the ssh pane comes beside the sidebar, and that machine’s tmux is asked to turn to the job — which is what makes one shared session show the row under the cursor rather than whatever it was left on.',
      },
      {
        type: 'p',
        text: 'Enter does the same thing, and lands you there: it focuses the ssh pane, then runs one command on that machine — the same pid → tty → pane resolution, then select-window / select-pane — so the far side turns to the job you picked instead of whatever it was showing. Over a live ControlMaster that lands in a few hundred milliseconds; it never blocks the sidebar, never asks for a password (BatchMode), and does nothing when the agent is not in a tmux pane there. If more than one client is attached to that remote tmux, the window is selected but no one’s view is dragged along — yours cannot be told from theirs.',
      },
      {
        type: 'callout',
        title: 'A remote pid is never matched locally',
        text: 'The pid and workspace path on a remote job describe another machine’s process and another machine’s filesystem, so neither is compared against a local pane — the same rule the hub follows before it reaps a job whose process is gone.',
      },
      { type: 'h2', text: 'The sidebar does not notify' },
      {
        type: 'p',
        text: 'OS banners are a per-surface, per-machine, user-toggled capability — the menu-bar app and the Windows tray raise them, the sidebar does not. It uses colour and the status-line `{ask}?` tally (Ask reasons at suggested+) — a soft “ready for you”, not an OS alert. `{attention}` still counts every needs-a-look row including system waits; the default format prefers `{ask}`.',
      },
      { type: 'h2', text: 'When the sidebar is not what you expect' },
      {
        type: 'table',
        headers: ['You see', 'It means'],
        rows: [
          ['No sidebar', 'Run prefix + e, or check nerve-tmux-surface is installed'],
          ['nerve: offline', 'Hub not reachable — it retries automatically'],
          ['nerve: no jobs', 'Hub is up but nothing is reporting'],
          ['… is another machine — no ssh pane here', 'That job runs elsewhere and no pane here is ssh’d into it. Open an ssh session to it (or run the sidebar on that machine — the tunnel already carries the jobs both ways)'],
          ['no local pane for …', 'The job runs on this machine, but its agent is not in any tmux pane — an IDE terminal, say'],
        ],
      },
      {
        type: 'p',
        text: 'The trampoline always exits 0 and never blocks tmux, whatever it fails to find — a status instrument that breaks your terminal is worse than no instrument. ./scripts/nerve.sh --verify-tmux proves the whole path end to end against an isolated tmux server, so it never touches the one you are using.',
      },
    ],
  },
  {
    slug: 'vscode',
    title: 'VS Code extension',
    lede: 'A third peer surface: the same hub stream, painted in the editor you are already in. Focus lands on a terminal or folder in this window — not another vscode:// hop.',
    blocks: [
      {
        type: 'callout',
        title: 'Display only',
        text: 'The extension does not run agents and does not own state. It holds one GET /v1/stream?surface=vscode. Local actions are Focus and Copy. There is no approve, cancel, or submit. It raises no OS banner — those belong to the platform-native surfaces, and every surface decides for itself. VS Code uses a gentle in-editor toast for Ask upgrades, a soft status-bar label, and an Activity Bar badge.',
      },
      { type: 'h2', text: 'Install from this repo' },
      {
        type: 'code',
        lang: 'bash',
        code: `cd vsc-ext
npm install
npm test
npm run watch`,
      },
      {
        type: 'p',
        text: 'Then Run and Debug → Run Nerve Extension (F5), or ./scripts/nerve.sh --vscode. The hub port is fixed at 127.0.0.1:17890; the extension will start nerve-hub if nothing answers, the same way tmux does, and will never kill it.',
      },
      { type: 'h2', text: 'What you see' },
      {
        type: 'table',
        headers: ['Chrome', 'What it is'],
        rows: [
          ['Status bar', 'Ambient label: Nerve · your turn, Nerve · running, Nerve · offline. Click opens the job view. Warning background only for hard Ask (required/urgent) or problems — suggested Ask stays text-only.'],
          ['Activity Bar → Nerve', 'Jobs grouped by machine alias. Colour dots match the macOS / tmux / site palette. This-folder jobs are marked here.'],
          ['Tooltip', 'Producer · alias · last prompt. The prompt never falls back to the activity summary.'],
          ['Badge', 'Ask (your turn / review / approval) + problem count. Tooltip: “N ready for you”. Not an OS banner.'],
          ['Ask toast', 'Optional (nerve.notifications.ask, default on). InformationMessage for suggested; WarningMessage only for required/urgent. Open runs Focus.'],
        ],
      },
      { type: 'h2', text: 'Commands' },
      {
        type: 'table',
        headers: ['Command', 'Action'],
        rows: [
          ['Nerve: Show Jobs', 'Focus the Nerve view'],
          ['Nerve: Focus Job…', 'Quick Pick the full hub set and jump'],
          ['Nerve: Copy Location / Copy Summary', 'Clipboard helpers'],
          ['Nerve: Reconnect', 'Drop and re-open the SSE stream'],
          ['Filter All / Attention / Running / This Folder', 'View title buttons. Default is All — a peer surface sees every machine.'],
        ],
      },
      { type: 'h2', text: 'Focus' },
      {
        type: 'p',
        text: 'A job on this machine is matched by extensions.pid against a VS Code terminal, then by workspace path. A hit on the current folder is “already here”: the row is highlighted, the extension does not pretend it can select a Claude Code chat session. A job on another machine never compares its pid or file:// path locally. The button reads Open on <alias> and opens a vscode-remote window when ~/.ssh/config can name that host; otherwise the breadcrumb is copied.',
      },
      {
        type: 'callout',
        title: 'Remote windows get this for free',
        text: 'A VS Code Remote-SSH window is another extension host talking to 127.0.0.1:17890 on that machine. If the machine already has a RemoteForward tunnel back to your Mac, those jobs are the same jobs. Nothing to configure in the extension.',
      },
      { type: 'h2', text: 'When the view is not what you expect' },
      {
        type: 'table',
        headers: ['You see', 'It means'],
        rows: [
          ['Nerve · offline', 'Hub not reachable — it retries. The editor is not blocked.'],
          ['Nerve: no jobs', 'Hub is up but nothing is reporting'],
          ['No jobs in this folder', 'This Folder filter is on; All still has rows'],
          ['Open on arrhenius1', 'That job is not on this machine. It will not open a local folder.'],
        ],
      },
      {
        type: 'p',
        text: './scripts/nerve.sh --verify-vscode runs the unit suite (status golden, frame parse, focus ranking, hub launch). It does not boot a VS Code UI.',
      },
    ],
  },
  {
    slug: 'tether',
    title: 'Tether plugin',
    lede: 'A compile-time Tether extension that paints the same nerve-hub stream as the macOS menu bar. One hub per machine; notifications go to one elected surface.',
    blocks: [
      {
        type: 'callout',
        title: 'One hub',
        text: 'Tether probes http://127.0.0.1:17890/v1/health before spawning. If Nerve.app (or tmux, or VS Code) already holds the port, this plugin attaches to that hub. A second nerve-hub that loses the bind exits as already-running. Producers still never spawn the hub.',
      },
      { type: 'h2', text: 'Install' },
      {
        type: 'p',
        text: 'The plugin lives in this repo at surfaces/tether and is registered in the Tether app composition root. Enable it in Tether → Settings → Extensions. Connect to a host, then click Nerve in the toolbar.',
      },
      {
        type: 'code',
        lang: 'bash',
        code: `swift test --package-path surfaces/tether`,
      },
      { type: 'h2', text: 'What you see' },
      {
        type: 'ul',
        items: [
          'One GET /v1/stream?surface=tether for the life of the enabled plugin — that connection is the refcount, not each workspace tab',
          'A job list (preferring the host you launched from when aliases match)',
          'Ask banners only when the hub’s notify lease names tether as owner',
        ],
      },
      { type: 'h2', text: 'Notifications' },
      {
        type: 'p',
        text: 'The hub counts open streams and, with policy single (default), elects one owner: macos, then tether, then windows, then vscode, then tmux. PUT /v1/notify { "policy": "all" } restores the old “every surface fires” behaviour. Settings in Nerve.app and in the Tether extension both write that route.',
      },
    ],
  },
  {
    slug: 'windows',
    title: 'Windows tray',
    lede: 'Windows has no menu bar, so Nerve lives in the notification area with a compact status panel behind it.',
    blocks: [
      { type: 'h2', text: 'What the icon says' },
      {
        type: 'p',
        text: 'A tray icon is 16 pixels square at 100% scaling, so the menu bar\u2019s continuous ribbon cannot fit. It folds instead: seven statuses into three stacked bands in fixed slots, so a band\u2019s position always means the same thing.',
      },
      {
        type: 'table',
        headers: ['Band', 'Colour', 'Means'],
        rows: [
          ['Top', 'Red or orange', 'Something needs you'],
          ['Middle', 'Blue', 'Work is happening'],
          ['Bottom', 'Violet or green', 'Work finished'],
        ],
      },
      {
        type: 'ul',
        items: [
          'An empty slot is dropped, so one status is one thick bar rather than three thin ones.',
          'The stack lengthens with how much is running \u2014 the same ladder the macOS ribbon uses, so one job never looks like forty.',
          'A single problem among forty running jobs keeps at least 12% of the width. That floor is the whole reason the ribbon exists.',
          'Idle is one short gray bar, centred. Offline is the last state faded, with a diagonal slash.',
          'The icon never animates. A blinking tray icon reads as malware, not as activity.',
        ],
      },
      {
        type: 'callout',
        title: 'Windows 11 hides new tray icons',
        text: 'They go into the overflow chevron by default. Drag it out once to keep it visible \u2014 otherwise Nerve is running and you will never see it.',
      },
      { type: 'h2', text: 'The panel' },
      {
        type: 'p',
        text: 'Click the tray icon to open the panel. Right-click for Open Nerve or Quit. The Windows UI uses native WinUI 3 controls and a standard draggable, resizable title bar. Rust binaries own the hub connection, status, grouping and notification policy. The gear button opens Settings; the power button quits Nerve. Both remain available when there are no jobs. Launching Nerve again opens the existing panel; starting at login keeps it hidden. Rows group by machine, priority or status. Clicking a row opens its prompt, location, Open and Copy, and the last five timeline entries.',
      },
      {
        type: 'ul',
        items: [
          'It dismisses on focus loss or Escape. No taskbar button and no Alt-Tab entry \u2014 it is a glance, not a window to manage.',
          'Use Up and Down to select a job; Enter or Space toggles its details. The selected row scrolls into view. Tab moves between controls.',
          'Drag the native title bar to move the panel, or its border/corner to resize it; its size is remembered next time. Escape, focus loss, the close button and Alt+F4 hide the panel. Use Quit in the tray menu or panel power button to exit Nerve.',
          'Hiding it never drops the stream. That connection is what keeps the hub alive, so it outlives any window.',
          'Open takes you to the job; Copy puts its line on the clipboard. There is no approve, cancel or submit \u2014 Nerve never reverse-controls an agent.',
          'A job on another machine is never opened locally: the same path very likely exists here too, and opening the wrong one silently is worse than opening nothing. Its location is copied instead, and the panel says why.',
        ],
      },
      { type: 'h2', text: 'Install' },
      {
        type: 'code',
        lang: 'powershell',
        code: `# from the extracted Windows download (no Rust required)
.\\scripts\\nerve.ps1 -Install -Run

# from a checkout (requires Rust, .NET 10 SDK and Windows build tools)
.\\scripts\\nerve.ps1 -Install
.\\scripts\\nerve.ps1 -Run`,
      },
      {
        type: 'p',
        text: '-Install copies the complete native UI package, including runtime DLLs/resources, nerve-windows-core.exe and nerve-hub.exe, to %LOCALAPPDATA%\\Programs\\Nerve and creates a Start Menu shortcut. Extract the whole download; users do not need Rust, .NET or a separately installed Windows App Runtime. From a checkout, the script builds and publishes to target\\windows first. To use a package in another folder, add -BinaryDirectory C:\\path\\to\\package. Quit Nerve and other connected surfaces before upgrading or uninstalling, then allow the hub 30 seconds to stop. -Uninstall keeps your preferences.',
      },
      {
        type: 'callout',
        title: 'SmartScreen',
        text: 'An unsigned executable warns on first run. That is expected until the binaries are signed.',
      },
      { type: 'h2', text: 'Notifications' },
      {
        type: 'p',
        text: 'Off by default. You very plausibly run the VS Code extension on the same machine, and surfaces are peers that cannot know about each other \u2014 so two banners for one Ask is a worse first impression than none. Turn them on from the panel gear button; Notification sound is a separate switch. The app registers its notification identity, but delivery still depends on Windows notification settings and unpackaged-app support.',
      },
      {
        type: 'ul',
        items: [
          'Only the Ask channel interrupts: input, approval, auth, permission, decision, elicitation, review, at suggested or above.',
          'Waiting on a lock, a queue or a dependency paints orange but never notifies \u2014 nothing you do right now would move it.',
          'The same job at the same level stays quiet for 120 seconds after firing.',
          'Clicking a banner takes you to the job.',
        ],
      },
      { type: 'h2', text: 'Start at login' },
      {
        type: 'p',
        text: 'Off by default, and available in the panel Settings when you want it. It writes HKCU\\...\\CurrentVersion\\Run, which appears in Task Manager \u2192 Startup \u2014 where you would look to remove it. Worth knowing: because the surface holds the stream open, autostart makes nerve-hub.exe permanently resident rather than a process that exits when you stop looking.',
      },
      { type: 'h2', text: 'What is not here' },
      {
        type: 'ul',
        items: [
          'The tmux sidebar. tmux has no Windows port \u2014 use WSL for that, or the VS Code extension.',
          'Raising an agent\u2019s own terminal window. Opening its location is what Focus means here.',
          'Clearing the hub from this surface: one peer wiping every peer\u2019s view is a product decision, not a port decision.',
        ],
      },
    ],
  },
  {
    slug: 'status',
    title: 'Status & lifecycle',
    lede: 'Display status is derived from structured facets only. One continuous ribbon; sessions leave on SessionEnd.',
    blocks: [
      { type: 'h2', text: 'Where status lives' },
      {
        type: 'p',
        text: 'The authority is nerve-hub, a small local daemon on fixed loopback 127.0.0.1:17890. Producers POST there; it holds every job and its timeline in memory and fans the whole set out to surfaces over SSE. The menu-bar app is a pure surface — it paints what the hub streams and derives no truth of its own, so the jobs outlive any single window.',
      },
      {
        type: 'ul',
        items: [
          'The port is fixed and not configurable: binding 17890 is itself the single-instance lock, so there is no port field in Settings (only the endpoint, shown for copying) and no NERVE_* env to set.',
          'The app auto-spawns the nerve-hub binary bundled inside Nerve.app when nothing answers on the port, and attaches to the running hub otherwise.',
          'Surfaces are reference-counted. When the last SSE subscriber disconnects, the hub waits out a grace period (30s by default) and exits on its own. Producer POSTs do not keep it alive.',
          'Every surface — menu bar, tmux plugin, plain curl — reads the same frames, so status never depends on which one you have open.',
        ],
      },
      { type: 'h2', text: 'Display statuses' },
      {
        type: 'table',
        headers: ['Status', 'Default', 'Meaning'],
        rows: [
          ['Problem', 'Red', 'Failed / cannot continue'],
          ['Attention', 'Orange', 'Needs you (input, approval) or stuck on the system'],
          ['Running', 'Blue', 'Actively executing (main thread, or a background shell/subagent)'],
          ['Monitor', 'Purple', 'Watching a background stream; not executing'],
          ['Success', 'Green', 'Ended success'],
          ['Inactive', 'Gray', 'Ready (no turn yet), paused, or unknown'],
        ],
      },
      {
        type: 'p',
        text: 'Six hues, five of them rainbow: red problem, orange attention, blue running, purple monitor, green success — plus gray idle. Waiting on the system shares orange with “needs you”. A background shell or subagent still executing is Running blue; only a monitor holding the stream is purple. Open sessions stay while lifecycle is active; SessionEnd removes the row. Your turn is Attention — continue in the agent UI. SessionStart is Ready/Inactive.',
      },
      { type: 'h2', text: 'Main-session lifecycle' },
      {
        type: 'table',
        headers: ['Phase', 'Facet', 'Ribbon'],
        rows: [
          ['Start', 'starting (“Ready”)', 'Inactive'],
          ['Work', 'thinking · tool', 'Running'],
          ['Your turn', 'idle · attention.reason=input', 'Attention'],
          ['Approval in agent', 'waiting · attention.reason=approval', 'Attention'],
          ['Background shell / subagent', 'subagent (+ background_tasks)', 'Running'],
          ['Background monitor only', 'monitor + outcome=partial', 'Monitor'],
          ['End', 'ended (+ endReason)', 'Leaves panel'],
        ],
      },
      { type: 'h2', text: 'Focus (A+B — not control)' },
      {
        type: 'ul',
        items: [
          'Snapshots carry location.openURL (IDE deep link when hosted in Cursor/VS Code, else workspace file://) and location.focusHint (producer · project · host · path).',
          'Panel primary action is Open / Focus — jumps to the agent workspace. Copy is secondary. No Approve / Type here.',
          'Notification click selects the job, expands it in the panel, and runs Open/Focus.',
          'Attention copy is honest and calm: “Your turn” / “A review is waiting” / “Approval needed in agent” — return to the agent UI to continue. No CRITICAL / stacked exclamation marks.',
          'Default interrupt channel is Ask reasons (input, review, decision, approval, …) at attention.level ≥ suggested. System waits only colour the ribbon. Suggested Ask banners stay silent even when Play sounds is on.',
        ],
      },
      { type: 'h2', text: 'How a session leaves the panel' },
      {
        type: 'ul',
        items: [
          'SessionEnd (clear, logout, prompt_input_exit, …) — authoritative.',
          'Same UI slot + new SessionStart — previous id ended as superseded (/new · /clear without SessionEnd).',
          'Local producer PID gone — app reaps as process_gone (closed terminal / kill).',
        ],
      },
      { type: 'h2', text: 'Design notes' },
      {
        type: 'ul',
        items: [
          'Jobs, not agents — machines are tunnels; producers report jobs.',
          'Memory-only runtime for jobs, timelines, and pending actions.',
          'Fail-open hooks; fixed loopback ingest; no project env vars.',
          'Out of scope: multi-display ribbons, cloud sync, disk history, in-app agent chat, submit_input / approve from Nerve.',
        ],
      },
    ],
  },
  {
    slug: 'ingest',
    title: 'Ingest API',
    lede: 'Loopback only: http://127.0.0.1:17890. The port is fixed, not a setting. Remotes reach it through SSH reverse tunnels.',
    blocks: [
      {
        type: 'p',
        text: 'This contract belongs to nerve-hub, a small local daemon that holds the jobs. Producers POST into it; the menu-bar app and the tmux plugin are surfaces that read from it. Any alias is accepted — there is no allow-list.',
      },
      {
        type: 'ul',
        items: [
          'The port is not a setting. 17890 is fixed because binding it is the single-instance lock, so producers can hard-code the address and there is no NERVE_* env to read. The macOS app shows the endpoint under Settings → Local Endpoint but offers no port field to change.',
          'You do not start the hub by hand. The menu-bar app spawns the nerve-hub binary bundled inside Nerve.app when nothing answers on the port, and attaches to the already-running hub otherwise.',
        ],
      },
      { type: 'h2', text: 'Logs' },
      {
        type: 'p',
        text: 'The hub and every surface write debug logs with standard libraries (Rust tracing, Apple Unified Logging, VS Code’s log output channel). Prompt text is never logged — only event names, producer keys, job ids, and counts.',
      },
      {
        type: 'ul',
        items: [
          'nerve-hub: stderr plus a daily file at ~/Library/Logs/Nerve/nerve-hub.log.YYYY-MM-DD (Linux: ~/.local/state/nerve/logs/, Windows: %LOCALAPPDATA%\\Nerve\\Logs\\). Default filter is info. `nerve-hub serve --verbose` or `RUST_LOG=nerve_hub=debug` for every hook event and surface attach/detach.',
          'macOS app and Tether: Console.app, subsystems app.nerve.Nerve and app.nerve.tether, category hub.',
          'tmux / Windows tray: same log directory, nerve-tmux.log / nerve-windows.log. RUST_LOG=nerve_surface_core=debug for frame-by-frame traces.',
          'VS Code: Output panel → Nerve (log channel).',
        ],
      },
      { type: 'h2', text: 'Endpoints' },
      {
        type: 'table',
        headers: ['Method', 'Path', 'Purpose'],
        rows: [
          ['GET', '/v1/health', 'Liveness'],
          ['GET', '/v1/jobs', 'Current jobs (in memory), each with its timeline'],
          ['POST', '/v1/refresh', 'Surface Refresh: reap dead local PIDs, expire pending, republish a frame, return the job list'],
          ['GET', '/v1/stream', 'SSE — full frames for surfaces'],
          ['POST', '/v1/snapshot', 'Full job snapshot(s) — requires alias'],
          ['POST', '/v1/events', 'Incremental events — requires alias when creating jobs'],
          ['POST', '/v1/demo', 'Built-in demo jobs'],
          ['POST', '/v1/clear', 'Clear in-memory jobs'],
          ['GET', '/v1/actions/pending?producerId=', 'Poll action queue'],
          ['POST', '/v1/actions/result?producerId=', 'Report action completion'],
        ],
      },
      { type: 'h2', text: 'Snapshot body' },
      {
        type: 'code',
        lang: 'json',
        code: `{
  "alias": "gpu-box",
  "machineKind": "linux",
  "jobs": [
    {
      "id": "claude-code:sess_1",
      "kind": "session",
      "name": "nerve",
      "alias": "gpu-box",
      "producer": {
        "id": "claude-code",
        "name": "Claude Code",
        "kind": "agent.claude"
      },
      "lifecycle": "active",
      "current": { "type": "thinking", "summary": "…" },
      "attention": { "level": "none" },
      "health": "ok",
      "progress": { "kind": "none" },
      "createdAt": "…",
      "updatedAt": "…",
      "version": 1
    }
  ]
}`,
      },
      {
        type: 'ul',
        items: [
          'alias — free-form machine label shown in the panel',
          'kind — job shape (session, build, test, …), not “agent”',
          'producer — who reported the job',
          'name — typically project basename (cwd); status lives in current / attention',
          'attention.level — none < informational < suggested < required < urgent. Surfaces derive display colour; they never invent status from free text.',
          'attention.reason — Ask (input, approval, auth, permission, decision, elicitation, review) may interrupt; Wait (resource, queue, failure, …) paints Attention but does not banner by default.',
        ],
      },
      { type: 'h2', text: 'Surface stream' },
      {
        type: 'p',
        text: 'GET /v1/stream is Server-Sent Events. Every frame carries the whole truth — there is no delta protocol.',
      },
      {
        type: 'code',
        lang: 'json',
        code: `{ "jobs": [ … ], "departed": [ … ], "notify": { "policy": "single", "owner": "macos", "surfaces": ["macos"], "watchers": 1 } }`,
      },
      {
        type: 'ul',
        items: [
          'jobs — the authoritative full set. The first frame arrives on connect.',
          'departed — terminal states of jobs evicted since the previous frame (lifecycle ended, endedAt, outcome). A hint so surfaces do not lose the ending; jobs stays the authority.',
          'notify — interrupt lease. policy is single (one elected owner) or all. owner is the surface label allowed to fire when policy is single. surfaces / watchers are the live connections. An older hub omits the key; surfaces then fire independently.',
          'Reconnecting is resyncing — drop the connection and the next first frame is the full set again.',
          'Every job in a frame has the same shape as one from /v1/jobs, including the timeline the hub keeps for it: up to 40 entries, heartbeats excluded.',
          '?surface=<label> names the connection for the notify lease. It does not filter jobs. GET/PUT /v1/notify reads or sets the policy.',
        ],
      },
      {
        type: 'callout',
        title: 'The stream is the lifecycle',
        text: 'An open subscription is a surface being present. nerve-hub counts them: when the last one disconnects it waits out a grace period (30s by default) and exits. Producer POSTs do not keep it alive.',
      },
      { type: 'h2', text: 'Action protocol (producers poll themselves)' },
      {
        type: 'code',
        lang: 'bash',
        code: `curl -s 'http://127.0.0.1:17890/v1/actions/pending?producerId=my-producer'
curl -s -X POST 'http://127.0.0.1:17890/v1/actions/result?producerId=my-producer' \\
  -H 'Content-Type: application/json' \\
  -d '{"id":"<pending-id>","state":"succeeded","message":"ok"}'`,
      },
      {
        type: 'p',
        text: 'There is no /v1/actions/invoke — the hub answers 404. Running an action (Open / Focus, Copy) is a surface capability: the hub echoes the actions a producer declares and derives none of its own.',
      },
      {
        type: 'p',
        text: 'Wire sample in the repo: fixtures/demo_snapshot.json — `./scripts/nerve.sh --demo` POSTs it to /v1/snapshot.',
      },
    ],
  },
  {
    slug: 'privacy',
    title: 'Privacy',
    lede: 'You do not have to trust the privacy claim, you can read the source that proves it.',
    blocks: [
      {
        type: 'ul',
        items: [
          'Jobs, timelines, and pending actions are memory-only for the nerve-hub process',
          'Your last prompt per job (extensions.lastPrompt, 400 characters) travels in the same loopback snapshot and lives in hub RAM only — it dies with the job and with the hub',
          'Quitting Nerve clears runtime state completely',
          'Only preferences (machines, colors, notifications, coach flags) use UserDefaults',
          'Tunnel Hosts prefer your existing ~/.ssh/config + known_hosts; Nerve’s managed block only adds RemoteForward',
          'Ingest never leaves 127.0.0.1',
          'Remotes only via SSH reverse tunnels you configure',
          'Hooks fail open — agents never block on Nerve',
        ],
      },
      {
        type: 'callout',
        text: 'There is no “save names/summaries” toggle because nothing is persisted for jobs. Demo and clear are HTTP ingest only.',
      },
    ],
  },
];

const zhPluginPage: DocPage = {
    slug: 'plugin',
    title: 'Agent 插件',
    lede: '一个市场插件会把会话生命周期作为任务报告给 Nerve。各家用各自的官方钩子类型。失败开放，不需要项目环境变量。',
    blocks: [
      {
        type: 'callout',
        title: '每个会话一个任务',
        text: '任务 id 是 {producer}:{session_id}（永远不追加 agent_id）。Subagent 不会成为面板中的独立行——它们只更新主会话的 current 切面。设置了 agent_id 时，subagent 内的工具噪声会被忽略。应用还会丢弃旧的子行（parentJobId / role=subagent / 多段 id）。Stop 表示等待输入；SessionEnd 会移除该行。如果主机在 /new · /clear · fork 时跳过 SessionEnd，同一 UI slot 下一个 SessionStart 会为之前的 session_id 发送 ended 快照。本地快照会携带 pid，因此关闭的终端可以自动清理。',
      },
      { type: 'h2', text: '安装' },
      {
        type: 'table',
        headers: ['Harness', '安装', 'Producer id'],
        rows: [
          [
            'Claude Code',
            '/plugin marketplace add Roy-Kid/nerve → /plugin install nerve@nerve',
            'claude-code',
          ],
          [
            'Codex',
            'codex plugin marketplace add Roy-Kid/nerve → codex plugin add nerve@nerve',
            'codex',
          ],
          ['Grok', '使用同一个兼容 Claude 的市场 / 插件', 'grok'],
        ],
      },
      { type: 'h3', text: 'Claude Code' },
      {
        type: 'code',
        lang: 'text',
        code: `/plugin marketplace add Roy-Kid/nerve
/plugin install nerve@nerve`,
      },
      { type: 'h3', text: 'Codex CLI' },
      {
        type: 'code',
        lang: 'bash',
        code: `codex plugin marketplace add Roy-Kid/nerve
codex plugin add nerve@nerve

# local development
codex plugin marketplace add /ABS/PATH/TO/nerve
codex plugin add nerve@nerve`,
      },
      {
        type: 'p',
        text: '交互式 Codex：/plugins → nerve → install。如果收到提示，请用 /hooks 信任钩子。',
      },
      { type: 'h2', text: '官方钩子类型' },
      {
        type: 'table',
        headers: ['主机', '官方类型', '实际做什么'],
        rows: [
          ['Claude Code', 'command（exec form）', '官方：`"command": "node", "args": ["${CLAUDE_PLUGIN_ROOT}/hooks/nerve.js"]`。原生 Node，不走 shell。'],
          ['Grok', 'command', '`node grok-post.js` 把事件 JSON POST 到 http://127.0.0.1:17890/v1/hook。Grok 的 `type: http` 会拦截 loopback（SSRF）；连接失败失败开放。'],
          ['Codex', 'command', '官方文档：python3 ${PLUGIN_ROOT}/hooks/…。Codex 没有 HTTP 钩子类型。脚本：hooks/nerve.py，async。'],
        ],
      },
      {
        type: 'p',
        text: 'Claude 在 Node 里映射后 POST /v1/snapshot。Codex 在 Python 里映射后 POST /v1/snapshot。Grok 的 command 钩子把原始事件 POST 到 /v1/hook，由 hub 映射。hub 没开时全部失败开放。官方生命周期事件都有槽位（Claude 不挂 WorktreeCreate/Remove：空钩子会让 worktree 创建失败）。没有切面映射的事件会触发并空操作，不会编造状态。',
      },
      { type: 'h2', text: '钩子会报告什么' },
      {
        type: 'table',
        headers: ['Hook', '主会话切面', '状态条'],
        rows: [
          ['SessionStart', 'active + starting (“Ready”)', '未活动'],
          ['UserPromptSubmit', 'active + thinking', '运行中'],
          ['PreToolUse / PostToolUse（主线程）', 'active + tool / subagent', '运行中'],
          ['SubagentStart', 'active + current.type=subagent', '运行中'],
          ['SubagentStop（没有其他后台工作）', 'active + thinking', '运行中'],
          ['Stop + shell/subagent 后台任务', 'current.type=subagent', '运行中'],
          ['Stop + 仅 monitor 后台任务', 'current.type=monitor + outcome=partial', '监视中'],
          ['Stop 空后台 / idle_prompt', 'attention.reason=input', '需要你'],
          ['idle_prompt + shell/agent toast', 'current.type=subagent', '运行中'],
          ['idle_prompt + monitor toast', 'current.type=monitor + partial', '监视中'],
          ['PermissionRequest / permission_prompt', 'attention.reason=approval', '需要你'],
          ['PreCompact', 'thinking（或仍在的后台任务）', '运行中 / 监视中'],
          ['PostCompact + 后台任务', 'subagent · monitor', '运行中 / 监视中'],
          ['PostCompact 无后台', 'idle · attention.reason=input', '需要你'],
          ['SessionEnd', 'ended + endReason (+ success|cancelled)', '离开面板'],
          ['SessionStart（同一 UI slot 下的新 id）', '之前的 id → ended (superseded)', '旧行离开'],
          ['其他 Notification（没有 type）', 'active + info', '运行中'],
        ],
      },
      {
        type: 'ul',
        items: [
          '失败开放：Nerve 未运行时，Claude/Grok 的 command 钩子是非阻塞连接失败，nerve-hub hook 以 0 退出。永远不阻塞 agent。',
          '无环境变量。接入 URL 固定为 http://127.0.0.1:17890/v1/hook。',
          'Slot 记在 hub 里（producer + workspace），不是临时文件，因此没有 SessionEnd 的 /new 仍然能关闭之前的 session_id。',
          'UserPromptSubmit 还会发送 extensions.lastPrompt（以及 lastPromptAt），截断为 400 个字符。只有这次钩子运行能看到该提示，因此 hub 会把它延续到后续快照中——每个 surface 都会显示它：tmux 侧边栏的 Prompt 面板、macOS 展开行里的 Prompt 块，以及 VS Code 树的 tooltip。',
          '本地操作：先是 Open / Focus，然后是 Copy。行通过 SessionEnd、slot supersede 或 PID reap 离开——没有 Dismiss / Approve / submit_input。',
          '每个快照都有 location.openURL + focusHint，因此面板可以跳回 agent UI。',
          'Alias = 自由形式的机器标签（在 macOS 上优先使用 Bonjour LocalHostName）。',
          '状态永远不会从自由文本推断——只使用事件名和结构化字段。',
        ],
      },
      { type: 'h2', text: '开发' },
      {
        type: 'code',
        lang: 'bash',
        code: `python3 sources/agents/tests/test_nerve_hook.py
node --test plugins/nerve/hooks/nerve.test.js
cargo test -p nerve-hub hook`,
      },
      {
        type: 'p',
        text: '请优先使用市场安装，而不是旧的 sources/agents/codex/hooks.json 模板。',
      },
    ],
  };

const zhMachinesPage: DocPage = {
  slug: 'machines',
  title: '机器与远程主机',
  lede: '设置 → 机器会管理 SSH 反向隧道，让远程机器的本地回环接入能到达正在运行 Nerve 的 Mac。',
  blocks: [
    {
      type: 'p',
      text: '设置中的机器只用于隧道。快照中任何自由形式的 alias 都会显示在面板中——没有允许列表。',
    },
    { type: 'h2', text: '在运行 Nerve 的 Mac 上' },
    {
      type: 'ol',
      items: [
        '打开设置 → 机器',
        '这台 Mac 始终存在（alias = hostname 短名 / Bonjour LocalHostName）',
        '远程列表从你的 ~/.ssh/config 加载——使用刷新控件重新读取 Hosts',
        '启用一个 Host，然后点击连接。Nerve 只会注入 RemoteForward；连接详情仍保留在你的 SSH 配置中',
      ],
    },
    {
      type: 'callout',
      title: 'known_hosts 与 OTP / 验证码集群',
      text: '主机密钥来自本地 known_hosts。对于 MFA/验证码：先在终端中运行 `ssh <alias>` 以启动 ControlMaster；可用时，连接会在该 master 上使用 `ssh -O forward`。',
    },
    {
      type: 'callout',
      title: 'Alias 提示',
      text: '使用远程主机的 hostname 短名作为 alias，这样钩子无需额外设置就能匹配。校园 DHCP hostname 经常与 Bonjour LocalHostName 不同——不匹配可能会丢弃快照。',
    },
    { type: 'h2', text: '在远程机器上' },
    {
      type: 'ol',
      items: [
        '安装 nerve 市场插件',
        '照常运行会话',
        '钩子会 POST 到 http://127.0.0.1:17890——隧道会把请求转发到运行 Nerve 的主机',
      ],
    },
    { type: 'h2', text: '面板里的远程任务' },
    {
      type: 'p',
      text: '来自另一台机器的任务会保留那台机器的 alias，它的工作区路径也属于那台机器——所以这里不会去本地打开它。面板的主操作会写明去向（“Open on arrhenius1”），改为打开一个到那台机器的 ssh 会话，交给你为 ssh: 注册的应用（默认是终端）。Host 取自你自己的 ~/.ssh/config，按名字与 alias 匹配，所以 ssh Arrhenius 能连到自称 arrhenius1 的机器。IDE 深链（cursor://、vscode://）仍按原样打开——它们自己会路由到各自的远程。如果配置里没有匹配的 Host，Nerve 会复制位置，而不是假装打开了什么。',
    },
    {
      type: 'p',
      text: 'tmux 侧边栏用自己的方式回答同一个问题：在远程任务上按 Enter，会跳到你已经 ssh 进那台机器的那个 pane。那个 pane 通向的是**机器**而不是某个任务——同一台机器上的每个任务都会解析到同一个 ssh 会话，所以把高亮移到远程任务上会同时做两件事：那个 ssh pane 被搬到侧边栏旁边，同时请那台机器的 tmux 切到该任务——这样一个共享的会话显示的才是光标所在的那一行，而不是它上次停在的地方。Enter 做同一件事并把你送过去：聚焦那个 pane，然后在那台机器上跑一条命令（同样的 pid → tty → pane，然后 select-window / select-pane）。有活的 ControlMaster 时几百毫秒内完成，不阻塞侧边栏、不会索要密码；如果 agent 不在对面的 tmux 里就什么也不做。两个 surface 都不会拿远程的 pid 或远程路径去和本地的东西匹配。',
    },
  ],
};

const zhTmuxPage: DocPage = {
  slug: 'tmux',
  title: 'tmux 插件',
  lede: '菜单栏和 tmux 读取同一份实时更新，因此两者无需相互依赖也能保持最新。',
  blocks: [
    {
      type: 'callout',
      title: '快速设置',
      text: '从仓库 checkout 出发，一条命令就会构建 helper、连好 tmux 入口并重新加载 tmux。',
    },
    {
      type: 'code',
      lang: 'bash',
      code: `./scripts/nerve.sh --build --tmux --tmux-reload`,
    },
    {
      type: 'p',
      text: 'nerve-tmux-surface 会打开一个侧边栏 pane（与 tmux-agent-sidebar 对齐）：状态筛选栏、可滚动的任务列表，以及可折叠的 Prompt/Git 面板——Prompt 会显示你最后向该 agent 提出的内容，这正是状态行无法告诉你的一件事。prefix + e 用来切换它。数据通过 SSE 来自 nerve-hub——与菜单栏应用读取的 frame 相同。不需要菜单栏，因此它也能在无界面的 Linux 机器上工作。',
    },
    { type: 'h2', text: '安装 helper' },
    {
      type: 'code',
      lang: 'bash',
      code: `cargo install --path crates/nerve-tmux-surface`,
    },
    {
      type: 'p',
      text: '这会把 nerve-tmux-surface 放到 ~/.cargo/bin。从 checkout 开发时，你完全不必安装——插件会先查找仓库自己的 target/release，然后依次查找 /opt/homebrew/bin、/usr/local/bin、~/.cargo/bin，最后是 PATH。helper 就是整个插件：install（按键、选项、hooks）、侧边栏 TUI，以及 toggle/close。nerve.tmux 只是一个 fail-open 跳板，找到该二进制并运行 install——接线逻辑里不再有 bash。',
    },
    { type: 'h2', text: '接入 tmux' },
    {
      type: 'code',
      lang: 'bash',
      code: `# ~/.tmux.conf — from a checkout
run-shell ~/src/nerve/surfaces/tmux/nerve.tmux

# ~/.tmux.conf — with TPM handling the clone and updates
set -g @plugin 'Roy-Kid/nerve'
run-shell ~/.tmux/plugins/nerve/surfaces/tmux/nerve.tmux`,
    },
    {
      type: 'callout',
      title: '为什么 TPM 需要第二行',
      text: 'TPM 只会自动 source 插件仓库根目录下的 *.tmux 文件。Nerve 是 monorepo，入口点位于 surfaces/tmux/nerve.tmux，因此 @plugin 负责 clone 和更新，run-shell 则指向入口点。如果你愿意自己 clone，只有 run-shell 那一行就够了。',
    },
    {
      type: 'p',
      text: '重复 source 是安全的：按键绑定会重新设置，当窗口中已有侧边栏时，再次切换不会执行任何操作。',
    },
    { type: 'h2', text: '按键' },
    {
      type: 'table',
      headers: ['按键', '操作'],
      rows: [
        ['prefix + e', '侧边栏循环：打开+聚焦 → 取消聚焦 → 关闭'],
        ['j / k', '移动选择；右侧 pane 预览该任务'],
        ['Enter', '跳转到该任务的真实 session/window/pane'],
        ['h / l / Tab', '循环切换状态筛选器'],
        ['Shift+Tab', '底部面板在 Prompt ⇄ Git 之间切换'],
        ['Ctrl-d / Ctrl-u, PgDn / PgUp', '滚动底部面板——选择始终留在任务列表中'],
        ['Space', '把面板折叠到标题行（点击标题效果相同）'],
      ],
    },
    { type: 'h2', text: '选项' },
    {
      type: 'table',
      headers: ['选项', '默认值', '用途'],
      rows: [
        ['@nerve_sidebar_width', '15%', '侧边栏宽度（列数或 %）'],
        ['@nerve_sidebar_position', 'left', 'left 或 right'],
        ['@nerve_sidebar_bottom_height', '20', '底部面板高度（0 表示隐藏）'],
        ['@nerve_sidebar_auto_create', 'on', '在新窗口中自动创建侧边栏'],
        ['@nerve_sidebar_key', 'e', 'prefix 之后的切换键'],
        ['@nerve_sidebar_close_key', 'q', 'prefix 之后的关闭键'],
      ],
    },
    {
      type: 'callout',
      title: '仅用于显示',
      text: 'Enter 会跳转到与该任务匹配的 tmux session/window/pane。这里没有 approve、cancel 或 submit，tmux 也永远不会打开 Finder 文件夹。',
    },
    { type: 'h2', text: '它与 hub 的关系' },
    {
      type: 'ul',
      items: [
        'helper 是 consumer，不是 authority。它会查找 nerve-hub（/opt/homebrew/bin → /usr/local/bin → ~/.cargo/bin → PATH）；如果 127.0.0.1:17890 没有响应，就启动一个，且最快每 10s 尝试一次。',
        '侧边栏 pane 只保持一条 GET /v1/stream?surface=tmux。这条打开的连接就是该 surface 的 hub refcount。',
        '每个窗口一个侧边栏（默认自动创建）。关闭该 pane，hub 就会失去一个 subscriber。',
        '任何内容都不会写入磁盘：没有 launchd job、systemd unit 或 state file。存在的少量状态位于 tmux options 中，它们会随 tmux server 一起消失。',
      ],
    },
    { type: 'h2', text: '远程主机无需额外配置' },
    {
      type: 'p',
      text: '如果一台机器已经有一条 RemoteForward 隧道连回你的 Mac（参见机器与远程主机），那么远程主机上的 127.0.0.1:17890 就已经是你 Mac 的 hub。在那里安装 helper、添加 run-shell 行，远程主机的 tmux 就会显示你的任务。helper 没有需要配置的“远程”概念——没有 host、port 或 env var。这与远程 agent 钩子 POST 到 loopback 却指向这台 Mac 是同一套设计。',
    },
    { type: 'h2', text: '侧栏不发系统通知' },
    {
      type: 'p',
      text: '系统横幅是「每个界面、每台机器、由用户自己开关」的能力——菜单栏应用和 Windows 托盘会发，侧边栏不发。它用颜色和状态行 `{ask}?` 计数（Ask reason 且 level ≥ suggested）做温柔的「轮到你」提示，而不是操作系统告警。`{attention}` 仍统计所有需要看一眼的行（含系统等待）；默认格式优先 `{ask}`。',
    },
    { type: 'h2', text: '当侧边栏不符合预期时' },
    {
      type: 'table',
      headers: ['你看到的内容', '它的含义'],
      rows: [
        ['没有侧边栏', '运行 prefix + e，或检查 nerve-tmux-surface 是否已安装'],
        ['nerve: offline', 'Hub 无法访问——它会自动重试'],
        ['nerve: no jobs', 'Hub 已启动，但没有任务正在报告'],
      ],
    },
    {
      type: 'p',
      text: '无论跳板找不到什么，它都会以 0 退出，永远不阻塞 tmux——会破坏终端的状态工具，还不如没有。./scripts/nerve.sh --verify-tmux 会针对一个隔离的 tmux server 端到端验证整条路径，因此它永远不会碰你正在使用的那一个。',
    },
  ],
};

const zhVscodePage: DocPage = {
  slug: 'vscode',
  title: 'VS Code 扩展',
  lede: '第三条对等显示面：同一条 hub 流，画在你已经待着的编辑器里。Focus 落到本窗口的 terminal 或文件夹，而不是再走一圈 vscode://。',
  blocks: [
    {
      type: 'callout',
      title: '仅用于显示',
      text: '扩展不跑 agent，也不拥有状态。它只保持一条 GET /v1/stream?surface=vscode。本地动作是 Focus 和 Copy。没有 approve、cancel 或 submit。它不发系统横幅——那属于平台原生界面，且每个界面各自决定。VS Code 对 Ask 升级用温和的编辑器内 toast，以及柔和的状态栏文案与 Activity Bar 徽章。',
    },
    { type: 'h2', text: '从仓库安装' },
    {
      type: 'code',
      lang: 'bash',
      code: `cd vsc-ext
npm install
npm test
npm run watch`,
    },
    {
      type: 'p',
      text: '然后 Run and Debug → Run Nerve Extension（F5），或 ./scripts/nerve.sh --vscode。hub 端口固定为 127.0.0.1:17890；没有响应时扩展会像 tmux 一样拉起 nerve-hub，并且永远不会杀掉它。',
    },
    { type: 'h2', text: '你会看到什么' },
    {
      type: 'table',
      headers: ['界面', '含义'],
      rows: [
        ['状态栏', '常驻文案：Nerve · your turn、Nerve · running、Nerve · offline。点击打开任务视图。仅 hard Ask（required/urgent）或 Problem 使用 warning/error 底色——suggested Ask 只改文字。'],
        ['Activity Bar → Nerve', '按机器 alias 分组。色点与 macOS / tmux / 站点调色板一致。当前文件夹里的任务标 here。'],
        ['Tooltip', 'producer · alias · 最后一条提示。提示词绝不回退成 activity 摘要。'],
        ['徽章', 'Ask（轮到你 / review / 批准）+ problem 条数。Tooltip：「N ready for you」。不是操作系统横幅。'],
        ['Ask toast', '可选（nerve.notifications.ask，默认开）。suggested 用 InformationMessage；仅 required/urgent 用 WarningMessage。Open 执行 Focus。'],
      ],
    },
    { type: 'h2', text: '命令' },
    {
      type: 'table',
      headers: ['命令', '作用'],
      rows: [
        ['Nerve: Show Jobs', '聚焦 Nerve 视图'],
        ['Nerve: Focus Job…', 'Quick Pick 全表并跳转'],
        ['Nerve: Copy Location / Copy Summary', '剪贴板'],
        ['Nerve: Reconnect', '断开并重连 SSE'],
        ['Filter All / Attention / Running / This Folder', '视图标题按钮。默认是 All——对等 surface 看见每一台机器。'],
      ],
    },
    { type: 'h2', text: 'Focus' },
    {
      type: 'p',
      text: '本机任务先用 extensions.pid 匹配 VS Code terminal，再用工作区路径。命中当前文件夹就是「已经在这里」：高亮该行，扩展不假装能选中某条 Claude Code 会话。外机任务的 pid 和 file:// 路径永不拿来和本机比。按钮文案是 Open on <alias>，能从 ~/.ssh/config 叫出 Host 时打开 vscode-remote 窗口，否则复制面包屑。',
    },
    {
      type: 'callout',
      title: '远程窗口无需额外配置',
      text: 'VS Code Remote-SSH 窗口是另一套 extension host，连的是那台机器上的 127.0.0.1:17890。如果那边已经有 RemoteForward 隧道回到你的 Mac，看到的就是同一批任务。扩展里没有要填的 host 或端口。',
    },
    { type: 'h2', text: '当视图不符合预期时' },
    {
      type: 'table',
      headers: ['你看到的内容', '它的含义'],
      rows: [
        ['Nerve · offline', 'Hub 不可达——它会重试。编辑器不会被挡住。'],
        ['Nerve: no jobs', 'Hub 在，但没有 producer 在报'],
        ['No jobs in this folder', '开了 This Folder 过滤；All 里还有行'],
        ['Open on arrhenius1', '该任务不在这台机器上。它不会打开本地文件夹。'],
      ],
    },
    {
      type: 'p',
      text: './scripts/nerve.sh --verify-vscode 跑单元测试（status golden、frame 解析、focus 排序、hub 拉起）。它不会启动 VS Code UI。',
    },
  ],
};

const zhTetherPage: DocPage = {
  slug: 'tether',
  title: 'Tether 插件',
  lede: '编译进 Tether 的扩展，画的是和 macOS 菜单栏同一条 nerve-hub 流。每台机器一个 hub；通知只发给被选出的那一个 surface。',
  blocks: [
    {
      type: 'callout',
      title: '只有一个 hub',
      text: 'Tether 先探测 http://127.0.0.1:17890/v1/health。若 Nerve.app（或 tmux、VS Code）已经占用该端口，插件就连那个 hub。第二个 nerve-hub 抢不到绑定会以 already-running 退出。Producer 仍然从不拉起 hub。',
    },
    { type: 'h2', text: '安装' },
    {
      type: 'p',
      text: '插件在本仓库 surfaces/tether，并在 Tether 应用的 composition root 注册。在 Tether → 设置 → 扩展 中启用。连上主机后，点工具栏里的 Nerve。',
    },
    {
      type: 'code',
      lang: 'bash',
      code: `swift test --package-path surfaces/tether`,
    },
    { type: 'h2', text: '你会看到什么' },
    {
      type: 'ul',
      items: [
        '插件启用期间只保持一条 GET /v1/stream?surface=tether——这条连接才是 refcount，不是每个工作区标签',
        '任务列表（能匹配 alias 时优先显示你从哪台主机打开的）',
        '只有 hub 的 notify 租约把 tether 选为 owner 时才发 Ask 横幅',
      ],
    },
    { type: 'h2', text: '通知' },
    {
      type: 'p',
      text: 'Hub 统计打开的 stream，在 policy 为 single（默认）时选出一个 owner：macos，然后 tether，然后 windows，然后 vscode，然后 tmux。PUT /v1/notify { "policy": "all" } 恢复旧的「每个 surface 都发」。Nerve.app 和 Tether 扩展的设置都写这条路由。',
    },
  ],
};

const zhWindowsPage: DocPage = {
  slug: 'windows',
  title: 'Windows 托盘',
  lede: 'Windows 没有菜单栏，所以 Nerve 位于通知区，点击即可打开紧凑的状态面板。',
  blocks: [
    { type: 'h2', text: '图标在说什么' },
    {
      type: 'p',
      text: '100% 缩放下托盘图标只有 16 像素见方，菜单栏那条连续状态条放不下。于是它折叠起来：七种状态收进三条固定槽位的堆叠色带，位置固定，所以某一条的含义永远不变。',
    },
    {
      type: 'table',
      headers: ['色带', '颜色', '含义'],
      rows: [
        ['上', '红或橙', '需要你'],
        ['中', '蓝', '正在干活'],
        ['下', '紫或绿', '已经结束'],
      ],
    },
    {
      type: 'ul',
      items: [
        '空的槽位直接丢掉，所以单一状态是一条粗带，而不是三条细带。',
        '堆叠的长度随活跃数量增长——与 macOS 状态条同一条阶梯，所以 1 个任务不会看起来像 40 个。',
        '40 个运行中夹 1 个问题，那一条至少占 12% 宽度。这个下限正是状态条存在的理由。',
        '空闲是一条居中的灰色短带；离线是上一个状态淡化并加一道斜杠。',
        '图标从不做动画。不停闪烁的托盘图标读起来像恶意软件，而不是活动。',
      ],
    },
    {
      type: 'callout',
      title: 'Windows 11 会藏起新的托盘图标',
      text: '它们默认进入溢出区的箭头里。拖出来一次即可常驻——否则 Nerve 在跑，而你永远看不见它。',
    },
    { type: 'h2', text: '面板' },
    {
      type: 'p',
      text: '点击托盘图标打开面板，右键菜单提供 Open Nerve 和 Quit。Windows 界面使用原生 WinUI 3 控件与可拖动、缩放的标准标题栏；Rust 二进制负责 hub 连接、状态、分组和通知规则。顶部齿轮打开设置，电源图标退出 Nerve；没有任务时也可使用。再次启动会唤起已有面板，开机自启则保持隐藏。任务按机器、优先级或状态分组。点一行展开 prompt、运行位置、Open 与 Copy，以及最近五条时间线。',
    },
    {
      type: 'ul',
      items: [
        '失去焦点或按 Esc 即消失。没有任务栏按钮，也不出现在 Alt-Tab 里——它是一瞥，不是需要管理的窗口。',
        '上下方向键选择任务，Enter 或空格切换详情；选中行会自动滚入视野。Tab 在控件之间移动。拖动原生标题栏移动面板，拖动窗口边框或角落调整大小，下次打开会保留尺寸。关闭按钮与 Alt+F4 只隐藏面板。',
        '从右键菜单选择 Quit 或点击顶部电源图标才会退出；Esc、失焦和 Alt+F4 只隐藏面板。隐藏面板绝不会断开数据流。那条连接正是让 hub 活着的东西，所以它比任何窗口活得久。',
        'Open 带你去那个任务，Copy 把它那一行放进剪贴板。没有 approve、cancel 或 submit——Nerve 从不反向控制 agent。',
        '别的机器上的任务绝不会在本地打开：同样的路径在这台机器上很可能也存在，静默打开错误的那个比什么都不打开更糟。它会改为复制位置，并告诉你为什么。',
      ],
    },
    { type: 'h2', text: '安装' },
    {
      type: 'code',
      lang: 'powershell',
      code: `# 从完整解压的下载包（无需 Rust 或 .NET）
.\\scripts\\nerve.ps1 -Install -Run

# 从仓库（需要 Rust、.NET 10 SDK 和 Windows 构建工具）
.\\scripts\\nerve.ps1 -Install
.\\scripts\\nerve.ps1 -Run`,
    },
    {
      type: 'p',
      text: '-Install 把完整原生 UI 包、运行时 DLL 和资源、nerve-windows-core.exe 及 nerve-hub.exe 复制到 %LOCALAPPDATA%\\Programs\\Nerve，并创建开始菜单快捷方式。请完整解压下载包，用户无需另装 Rust、.NET 或 Windows App Runtime。从仓库构建时会先发布到 target\\windows。其他包目录可用 -BinaryDirectory 指定。升级或卸载前请退出所有连接的界面并等待 hub 停止；-Uninstall 保留偏好设置。',
    },
    {
      type: 'callout',
      title: 'SmartScreen',
      text: '未签名的可执行文件首次运行会触发警告。在二进制被签名之前，这是预期行为。',
    },
    { type: 'h2', text: '系统通知' },
    {
      type: 'p',
      text: '默认关闭。你很可能在同一台机器上也装了 VS Code 扩展，而各个界面互为对等、彼此并不知晓——所以同一个 Ask 弹两次通知，比一次都不弹的第一印象更差。在面板顶部的齿轮设置 中开启；通知声音可单独设置。',
    },
    {
      type: 'ul',
      items: [
        '只有 Ask 通道会打断：input、approval、auth、permission、decision、elicitation、review，且等级 ≥ suggested。',
        '等锁、等队列、等依赖同样显示为橙色，但绝不通知——你此刻做什么都推不动它。',
        '同一任务同一等级，发出后 120 秒内保持安静。',
        '点击通知会带你去那个任务。',
      ],
    },
    { type: 'h2', text: '开机自启' },
    {
      type: 'p',
      text: '默认关闭，需要时从面板顶部的齿轮设置 打开。它写入 HKCU\\...\\CurrentVersion\\Run，会出现在任务管理器 → 启动项里，也就是你想关掉它时会去找的地方。值得知道的是：因为界面会一直持有数据流，开启自启会让 nerve-hub.exe 变成常驻进程，而不是你不看时就退出的那种。',
    },
    { type: 'h2', text: '这里没有什么' },
    {
      type: 'ul',
      items: [
        'tmux 侧栏。tmux 没有 Windows 原生版——那种场景用 WSL，或者用 VS Code 扩展。',
        '把 agent 自己的终端窗口抬到前台。在这里，Focus 的含义就是打开它的位置。',
        '从这个界面清空 hub：一个对等界面抹掉所有界面的视图，是产品决策而不是移植决策。',
      ],
    },
  ],
};

const zhStatusPage: DocPage = {
  slug: 'status',
  title: '状态与生命周期',
  lede: '显示状态只从结构化切面推导。一条连续状态条；会话在 SessionEnd 时离开。',
  blocks: [
    { type: 'h2', text: '状态存在哪里' },
    {
      type: 'p',
      text: '权威来源是 nerve-hub：一个运行在固定本地回环 127.0.0.1:17890 上的小型守护进程。Producer 向它 POST；它在内存中保存每个任务及其时间线，并通过 SSE 把整个集合广播给 surface。菜单栏应用是纯 surface——它只绘制 hub 推送的内容，不会自行推导事实，因此任务的寿命不受任何单一窗口影响。',
    },
    {
      type: 'ul',
      items: [
        '端口固定且不可配置：绑定 17890 本身就是单实例锁，因此设置中没有端口字段（只显示可复制的 endpoint），也没有要设置的 NERVE_* env。',
        '当端口没有响应时，应用会自动启动 Nerve.app 中捆绑的 nerve-hub binary；否则就连接已运行的 hub。',
        'Surface 使用引用计数。最后一个 SSE subscriber 断开后，hub 会等待一段宽限时间（默认 30s），然后自行退出。Producer POST 不会让它保持运行。',
        '每个 surface——菜单栏、tmux 插件、普通 curl——都读取同一份 frame，因此状态永远不依赖你打开了哪一个 surface。',
      ],
    },
    { type: 'h2', text: '显示状态' },
    {
      type: 'table',
      headers: ['状态', '默认颜色', '含义'],
      rows: [
        ['Problem', '红色', '失败 / 无法继续'],
        ['Attention', '橙色', '需要你（输入、批准），或卡在系统上'],
        ['Running', '蓝色', '正在执行（主线程，或后台 shell/subagent）'],
        ['Monitor', '紫色', '正在监视后台输出，本身不再执行'],
        ['Success', '绿色', '已成功结束'],
        ['Inactive', '灰色', '就绪（尚未开始 turn）、已暂停或未知'],
      ],
    },
    {
      type: 'p',
      text: '六种颜色，其中五种是彩虹：红 Problem、橙 Attention、蓝 Running、紫 Monitor、绿 Success——加上灰 Inactive。等系统与“需要你”共用橙色。后台 shell / subagent 仍在执行是蓝色 Running；只有盯着 stream 的 monitor 是紫色。lifecycle 为 active 时，打开的会话留在面板中；SessionEnd 时立刻移除。轮到你时是 Attention——请在 agent UI 中继续。SessionStart 是 Ready/Inactive。',
    },
    { type: 'h2', text: '主会话生命周期' },
    {
      type: 'table',
      headers: ['阶段', 'Facet', '状态条'],
      rows: [
        ['开始', 'starting (“Ready”)', 'Inactive'],
        ['工作', 'thinking · tool', 'Running'],
        ['轮到你', 'idle · attention.reason=input', 'Attention'],
        ['在 agent 中批准', 'waiting · attention.reason=approval', 'Attention'],
        ['后台 shell / subagent', 'subagent (+ background_tasks)', 'Running'],
        ['仅后台 monitor', 'monitor + outcome=partial', 'Monitor'],
        ['结束', 'ended (+ endReason)', '离开面板'],
      ],
    },
    { type: 'h2', text: '聚焦（A+B——不是控制）' },
    {
      type: 'ul',
      items: [
        '快照会携带 location.openURL（宿主为 Cursor/VS Code 时是 IDE deep link，否则是工作区 file://）以及 location.focusHint（producer · project · host · path）。',
        '面板的主操作是 Open / Focus——跳转到 agent 工作区。Copy 是次要操作。没有 Approve / Type here。',
        '点击通知会选中任务、在面板中展开它，并执行 Open/Focus。',
        'Attention 文案冷静如实：“Your turn” / “A review is waiting” / “Approval needed in agent”——返回 agent UI 继续。不用 CRITICAL 或叠感叹号。',
        '默认打断通道是 Ask reason（input、review、decision、approval 等）且 attention.level ≥ suggested。系统等待只上色。Suggested Ask 横幅即使开了 Play sounds 也保持静音。',
      ],
    },
    { type: 'h2', text: '会话如何离开面板' },
    {
      type: 'ul',
      items: [
        'SessionEnd（clear、logout、prompt_input_exit、…）——权威信号。',
        '同一 UI slot + 新 SessionStart——之前的 id 会以 superseded 结束（没有 SessionEnd 的 /new · /clear）。',
        '本地 producer PID 消失——应用会以 process_gone 清理（关闭终端 / kill）。',
      ],
    },
    { type: 'h2', text: '设计说明' },
    {
      type: 'ul',
      items: [
        '对象是任务，不是 agent——机器是隧道；producer 报告任务。',
        '任务、时间线和 pending actions 的运行时数据只存在内存中。',
        '钩子失败开放；固定本地回环接入；没有项目 env vars。',
        '范围外：多显示器状态条、云同步、磁盘历史、应用内 agent chat、从 Nerve 执行 submit_input / approve。',
      ],
    },
  ],
};

const zhIngestPage: DocPage = {
  slug: 'ingest',
  title: '接入 API',
  lede: '仅本地回环：http://127.0.0.1:17890。端口是固定的，不是设置项。远程机器通过 SSH 反向隧道访问它。',
  blocks: [
    {
      type: 'p',
      text: '这份契约属于 nerve-hub：一个保存任务的小型本地守护进程。Producer 向它 POST；菜单栏应用和 tmux 插件是从它读取的 surface。任何 alias 都会被接受——没有允许列表。',
    },
    {
      type: 'ul',
      items: [
        '端口不是设置项。17890 是固定的，因为绑定它本身就是单实例锁；因此 producer 可以硬编码该地址，也没有要读取的 NERVE_* env。macOS 应用会在设置 → 本地端点下显示 endpoint，但不提供可修改的端口字段。',
        '你不需要手动启动 hub。当端口没有响应时，菜单栏应用会启动 Nerve.app 中捆绑的 nerve-hub binary；否则就连接已运行的 hub。',
      ],
    },
    { type: 'h2', text: '日志' },
    {
      type: 'p',
      text: 'hub 和每个 surface 用主流日志库写调试日志（Rust tracing、Apple Unified Logging、VS Code 的 log output channel）。不会记录 prompt 正文，只记事件名、producer、job id 和计数。',
    },
    {
      type: 'ul',
      items: [
        'nerve-hub：stderr，外加每日文件 ~/Library/Logs/Nerve/nerve-hub.log.YYYY-MM-DD（Linux：~/.local/state/nerve/logs/，Windows：%LOCALAPPDATA%\\Nerve\\Logs\\）。默认 info。`nerve-hub serve --verbose` 或 `RUST_LOG=nerve_hub=debug` 可看到每条 hook 和 surface 的 attach/detach。',
        'macOS 应用和 Tether：Console.app，subsystem 分别为 app.nerve.Nerve 和 app.nerve.tether，category hub。',
        'tmux / Windows 托盘：同一日志目录下的 nerve-tmux.log / nerve-windows.log。`RUST_LOG=nerve_surface_core=debug` 可看到每一帧。',
        'VS Code：Output 面板 → Nerve（log channel）。',
      ],
    },
    { type: 'h2', text: '端点' },
    {
      type: 'table',
      headers: ['Method', 'Path', '用途'],
      rows: [
        ['GET', '/v1/health', '存活检查'],
        ['GET', '/v1/jobs', '当前任务（内存中），每个任务都带有时间线'],
        ['POST', '/v1/refresh', 'Surface 刷新：回收已死的本机 PID、过期 pending、重发一帧，返回任务列表'],
        ['GET', '/v1/stream', 'SSE——向 surface 发送完整 frame'],
        ['POST', '/v1/snapshot', '完整任务快照——需要 alias'],
        ['POST', '/v1/events', '增量事件——创建任务时需要 alias'],
        ['POST', '/v1/demo', '内置演示任务'],
        ['POST', '/v1/clear', '清除内存中的任务'],
        ['GET', '/v1/actions/pending?producerId=', '轮询 action queue'],
        ['POST', '/v1/actions/result?producerId=', '报告 action 完成'],
      ],
    },
    { type: 'h2', text: '快照请求体' },
    {
      type: 'code',
      lang: 'json',
      code: `{
  "alias": "gpu-box",
  "machineKind": "linux",
  "jobs": [
    {
      "id": "claude-code:sess_1",
      "kind": "session",
      "name": "nerve",
      "alias": "gpu-box",
      "producer": {
        "id": "claude-code",
        "name": "Claude Code",
        "kind": "agent.claude"
      },
      "lifecycle": "active",
      "current": { "type": "thinking", "summary": "…" },
      "attention": { "level": "none" },
      "health": "ok",
      "progress": { "kind": "none" },
      "createdAt": "…",
      "updatedAt": "…",
      "version": 1
    }
  ]
}`,
    },
    {
      type: 'ul',
      items: [
        'alias——显示在面板中的自由形式机器标签',
        'kind——任务形态（session、build、test、…），不是“agent”',
        'producer——报告任务的一方',
        'name——通常是项目 basename（cwd）；状态存在于 current / attention',
        'attention.level——none < informational < suggested < required < urgent。表面从分面派生显示色，从不靠自由文本推断状态。',
        'attention.reason——Ask（input、approval、auth、permission、decision、elicitation、review）可打断；Wait（resource、queue、failure 等）只上色为 Attention，默认不弹横幅。',
      ],
    },
    { type: 'h2', text: 'Surface stream' },
    {
      type: 'p',
      text: 'GET /v1/stream 使用 Server-Sent Events。每个 frame 都携带完整事实——没有 delta protocol。',
    },
    {
      type: 'code',
      lang: 'json',
      code: `{ "jobs": [ … ], "departed": [ … ], "notify": { "policy": "single", "owner": "macos", "surfaces": ["macos"], "watchers": 1 } }`,
    },
    {
      type: 'ul',
      items: [
        'jobs——权威的完整集合。连接时会收到第一个 frame。',
        'departed——自上一个 frame 以来已移除任务的终止状态（lifecycle ended、endedAt、outcome）。它是一个提示，让 surface 不会丢失结尾；jobs 仍是权威来源。',
        'notify——打断租约。policy 为 single（选出一个 owner）或 all。owner 是 single 时允许发通知的 surface 标签。surfaces / watchers 是当前连接。旧版 hub 没有这个键，各 surface 各自决定。',
        '重新连接就是重新同步——断开连接后，下一个第一 frame 仍然是完整集合。',
        'frame 中的每个任务都与 /v1/jobs 返回的任务形状相同，包括 hub 为它保存的时间线：最多 40 个条目，不包含 heartbeat。',
        '?surface=<label> 为 notify 租约命名这条连接，不会过滤 jobs。GET/PUT /v1/notify 读取或设置 policy。',
      ],
    },
    {
      type: 'callout',
      title: 'Stream 就是生命周期',
      text: '一个打开的 subscription 就表示一个 surface 存在。nerve-hub 会对它们计数：当最后一个断开时，它会等待宽限期（默认 30s），然后退出。Producer POST 不会让它保持运行。',
    },
    { type: 'h2', text: 'Action 协议（producer 自行轮询）' },
    {
      type: 'code',
      lang: 'bash',
      code: `curl -s 'http://127.0.0.1:17890/v1/actions/pending?producerId=my-producer'
curl -s -X POST 'http://127.0.0.1:17890/v1/actions/result?producerId=my-producer' \\
  -H 'Content-Type: application/json' \\
  -d '{"id":"<pending-id>","state":"succeeded","message":"ok"}'`,
    },
    {
      type: 'p',
      text: '不存在 /v1/actions/invoke——hub 会回答 404。执行 action（Open / Focus、Copy）是 surface capability：hub 只回显 producer 声明的 actions，自己不会推导任何 action。',
    },
    {
      type: 'p',
      text: '仓库中的 wire sample：fixtures/demo_snapshot.json——`./scripts/nerve.sh --demo` 会将它 POST 到 /v1/snapshot。',
    },
  ],
};

const zhPrivacyPage: DocPage = {
  slug: 'privacy',
  title: '隐私',
  lede: '你不必盲目相信隐私声明，可以直接阅读证明它的源码。',
  blocks: [
    {
      type: 'ul',
      items: [
        '任务、时间线和 pending actions 只存在于 nerve-hub 进程内存中',
        '每个任务的最后一条 prompt（extensions.lastPrompt，400 个字符）会在同一个本地回环快照中传输，且只存在于 hub RAM——它会随任务和 hub 一起消失',
        '退出 Nerve 会完全清除运行时状态',
        '只有偏好设置（machines、colors、notifications、coach flags）使用 UserDefaults',
        '隧道 Hosts 优先使用你现有的 ~/.ssh/config + known_hosts；Nerve 的受管块只会添加 RemoteForward',
        '接入永远不会离开 127.0.0.1',
        '远程主机只通过你配置的 SSH 反向隧道连接',
        '钩子失败开放——agent 永远不会因 Nerve 而阻塞',
      ],
    },
    {
      type: 'callout',
      text: '没有“保存名称/摘要”开关，因为任务数据根本不会持久化。Demo 和 clear 也只是 HTTP ingest。',
    },
  ],
};

const zhDocPages: DocPage[] = [
  {
    slug: 'get-started',
    title: '开始使用',
    lede: 'Nerve 通过开放的本地回环协议，汇总 agent 已经在推送的状态；你也可以自己实现这套协议。',
    blocks: [
      { type: 'h2', text: '系统要求' },
      {
        type: 'p',
        text: 'hub 和钩子插件在哪都能跑。你拿到哪种界面，取决于这台机器。',
      },
      {
        type: 'table',
        headers: ['机器', '界面', '需要'],
        rows: [
          ['macOS 14+', '菜单栏色带', '从源码构建应用需要 Xcode 15+'],
          ['Windows 10/11', '托盘图标 + 弹出面板', '除了二进制文件之外无需其他'],
          ['Linux，或以上任意一种', 'tmux 侧栏 · VS Code 扩展', 'tmux 3.x · VS Code 1.90+'],
        ],
      },
      {
        type: 'ul',
        items: [
          'Rust 1.82+（从源码构建 nerve-hub 时需要）',
          'Node 18+（Claude 钩子插件）或 Python 3（Codex 钩子插件）',
          'OpenSSH 客户端（用于远程机器）',
        ],
      },
      { type: 'h2', text: '从仓库运行' },
      {
        type: 'code',
        lang: 'bash',
        code: `./scripts/nerve.sh --run

# optional: demo jobs (hub must already be running)
./scripts/nerve.sh --demo

# optional smoke checks
./scripts/nerve.sh --verify-loop`,
      },
      {
        type: 'p',
        text: 'macOS 上菜单栏会出现一条连续状态条——没有 Dock 图标，也没有悬浮窗口。Windows 上同样的状态折叠进通知区图标，点击后打开紧凑的状态面板。',
      },
      { type: 'h2', text: '状态条操作' },
      {
        type: 'table',
        headers: ['操作', '效果'],
        rows: [
          ['左键点击', '打开状态面板（↑/↓，Enter 展开；展开行会显示最后一条提示、详情、时间线和操作）'],
          ['右键点击', '打开设置…或退出 Nerve'],
        ],
      },
      {
        type: 'p',
        text: '设置：通用（菜单栏、面板、端点）· 机器（SSH 隧道）· 外观（状态条尺寸与颜色）· 通知 · 关于。',
      },
      { type: 'h2', text: '网站（本站）' },
      {
        type: 'code',
        lang: 'bash',
        code: `cd index
npm install
npm run dev      # local preview
npm run build    # static → index/dist/`,
      },
    ],
  },
  zhPluginPage,
  zhMachinesPage,
  zhTmuxPage,
  zhVscodePage,
  zhTetherPage,
  zhWindowsPage,
  zhStatusPage,
  zhIngestPage,
  zhPrivacyPage,
];

export const docNavByLocale: Record<Locale, DocNavItem[]> = {
  en: docNav,
  zh: zhDocNav,
};

export const docPagesByLocale: Record<Locale, DocPage[]> = {
  en: docPages,
  zh: zhDocPages,
};

export function getDocNav(locale: Locale): DocNavItem[] {
  return docNavByLocale[locale];
}

export function getDocPage(slug: string, locale: Locale = 'en'): DocPage | undefined {
  return docPagesByLocale[locale].find((p) => p.slug === slug);
}
