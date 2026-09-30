#!/usr/bin/env node
/**
 * Claude Code native hook (official exec form: `node` + args).
 * stdin JSON → one main-session snapshot → POST http://127.0.0.1:17890/v1/snapshot
 * Always exit 0.
 */
"use strict";

const crypto = require("crypto");
const fs = require("fs");
const http = require("http");
const os = require("os");
const path = require("path");
const { spawnSync } = require("child_process");

const INGEST_HOST = "127.0.0.1";
const INGEST_PORT = 17890;
// Connect + write only. The hook never reads a response, so this bounds a
// black-holed port, not how long the hub takes to answer.
const INGEST_TIMEOUT_MS = 250;
const PROMPT_MAX = 400;
const SUBAGENT_NOISE = new Set([
  "pretooluse",
  "posttooluse",
  "posttoolusefailure",
  "userpromptsubmit",
]);
const SUBAGENT_TOOLS = new Set([
  "spawn_subagent",
  "get_command_or_subagent_output",
  "Task",
  "Agent",
]);
const CANCELLED = new Set([
  "clear",
  "logout",
  "prompt_input_exit",
  "bypass_permissions_disabled",
  "resume",
  "superseded",
  "process_gone",
  "aborted",
  "dismissed",
]);
const META = {
  claude: { id: "claude-code", name: "Claude Code", kind: "agent.claude" },
};

function get(obj, keys, fallback) {
  if (!obj || typeof obj !== "object") return fallback;
  for (const k of keys) {
    const v = obj[k];
    if (v !== undefined && v !== null && v !== "") return v;
  }
  return fallback;
}

function eventName(payload) {
  const raw = String(get(payload, ["hook_event_name", "hookEventName", "event", "event_name"], "") || "");
  return raw.replace(/[_-\s]/g, "").toLowerCase();
}

function sessionId(payload) {
  const sid = get(payload, ["session_id", "sessionId", "conversation_id", "conversationId", "thread_id", "threadId"]);
  if (sid) return String(sid);
  const cwd = cwdOf(payload);
  return `anon-${Math.abs(hashCode(cwd)) % 10000000}`;
}

function hashCode(s) {
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (Math.imul(31, h) + s.charCodeAt(i)) | 0;
  return h;
}

function cwdOf(payload) {
  const cwd = get(payload, ["cwd", "working_directory"]);
  if (cwd) return String(cwd);
  const roots = payload.workspace_roots || payload.workspaceRoots;
  if (Array.isArray(roots) && roots[0]) return String(roots[0]);
  return process.cwd();
}

function truncate(s, n) {
  if (!s) return "";
  const t = String(s).split(/\s+/).join(" ");
  if (t.length <= n) return t;
  return t.slice(0, n - 1) + "…";
}

function nowIso() {
  return new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
}

function versionMs() {
  return Date.now();
}

let _alias;
function machineAlias() {
  if (_alias) return _alias;
  const cached = identityLoad();
  if (cached && cached.alias) {
    _alias = String(cached.alias);
    return _alias;
  }
  if (process.platform === "darwin") {
    try {
      const out = spawnSync("/usr/sbin/scutil", ["--get", "LocalHostName"], {
        encoding: "utf8",
        timeout: 1000,
        stdio: ["ignore", "pipe", "ignore"],
      });
      const name = (out.stdout || "").trim();
      if (out.status === 0 && name) {
        _alias = name;
        return _alias;
      }
    } catch (_) {
      /* fall through */
    }
  }
  _alias = (os.hostname() || "local").split(".")[0] || "local";
  return _alias;
}

function machineKind() {
  if (process.platform === "darwin") return "darwin";
  if (process.platform === "linux") {
    const rel = os.release().toLowerCase();
    if (rel.includes("microsoft") || rel.includes("wsl")) return "wsl";
    return "linux";
  }
  if (process.platform === "win32") return "windows";
  return "unknown";
}

function bgTasks(payload) {
  const raw = payload.background_tasks ?? payload.backgroundTasks;
  return Array.isArray(raw) ? raw : [];
}

function taskKind(task) {
  if (!task || typeof task !== "object") return "subagent";
  const raw = String(task.type || task.kind || "").toLowerCase();
  if (!raw) {
    const desc = String(task.description || task.name || task.agent_type || task.agentType || "").toLowerCase();
    if (desc.includes("monitor")) return "monitor";
    if (desc.includes("shell") || desc.includes("bash")) return "shell";
    return "subagent";
  }
  if (raw.includes("monitor")) return "monitor";
  if (raw === "shell" || raw === "bash" || raw === "command" || raw === "local_shell" || raw === "powershell" || raw.includes("shell")) {
    return "shell";
  }
  return "subagent";
}

function runningBg(summary, name) {
  return {
    lifecycle: "active",
    current: { type: "subagent", name, summary, startedAt: nowIso() },
    attention: { level: "none" },
    health: "ok",
  };
}

function monitorWait(summary, name) {
  return {
    lifecycle: "active",
    outcome: "partial",
    current: { type: "monitor", name, summary, startedAt: nowIso() },
    attention: { level: "none" },
    health: "ok",
  };
}

// Compact kind label for the row: one work kind, or "mixed" when they differ.
function bgKindLabel(kinds) {
  const uniq = [...kinds];
  if (!uniq.length) return "subagent";
  if (uniq.length > 1) return "mixed";
  return uniq[0] === "monitor" || uniq[0] === "shell" ? uniq[0] : "subagent";
}

function backgroundWork(tasks) {
  const n = tasks.length;
  const kinds = new Set(tasks.map(taskKind));
  const name = bgKindLabel(kinds);
  // List every task, not just the first: one summary is all a row has.
  const descs = tasks
    .map((t) => (t && typeof t === "object"
      ? String(t.description || t.name || t.agent_type || t.agentType || t.type || t.kind || "")
      : ""))
    .filter(Boolean)
    .map((d) => truncate(d, 40))
    .filter(Boolean);
  let summary = `${n} background task(s)`;
  if (descs.length) summary = `${summary}: ${truncate(descs.join(", "), 120)}`;
  if (kinds.size && [...kinds].every((k) => k === "monitor")) return monitorWait(summary, name);
  return runningBg(summary, name);
}

function classifyToast(text) {
  const s = String(text || "").toLowerCase().split(/\s+/).join(" ");
  if (!s) return null;
  const hasShell = s.includes("shell");
  const hasMonitor = s.includes("monitor");
  const hasAgent = s.includes("agent");
  const still = s.includes("still running") || s.includes("still run");
  const waiting = s.includes("waiting") || s.includes("finish") || s.includes("running");
  if (still && (hasShell || hasMonitor)) return hasMonitor && !hasShell && !hasAgent ? "monitor" : "running";
  if (hasAgent && waiting && (s.includes("background") || s.includes("waiting for") || s.includes("finish"))) return "running";
  if (s.includes("waiting for") && s.includes("background") && s.includes("task")) return "running";
  if (hasMonitor && waiting && !hasShell && !hasAgent) return "monitor";
  if (hasShell && waiting) return "running";
  return null;
}

function stopHookActive(payload) {
  const v = payload.stopHookActive ?? payload.stop_hook_active;
  return v === true || String(v).toLowerCase() === "true";
}

function stopFacets(payload) {
  const reason = String(get(payload, ["reason"], "") || "").trim();
  if (reason === "channel_closed" || reason === "shutdown") return null;
  if (stopHookActive(payload)) {
    return { lifecycle: "active", current: { type: "thinking", summary: "Continuing", startedAt: nowIso() }, attention: { level: "none" }, health: "ok" };
  }
  const bg = bgTasks(payload);
  if (bg.length) return backgroundWork(bg);
  return { lifecycle: "active", current: { type: "completed", summary: "Turn complete" }, attention: { level: "none" }, health: "ok" };
}

function yourTurn(summary) {
  return {
    lifecycle: "active",
    current: { type: "idle", summary: "Your turn — continue in the agent UI" },
    attention: {
      level: "suggested",
      reason: "input",
      title: "Your turn in agent",
      summary,
    },
    health: "ok",
  };
}

function mapEvent(event, payload) {
  if (event === "sessionstart") {
    return { lifecycle: "active", current: { type: "starting", summary: "Ready", startedAt: nowIso() }, attention: { level: "none" }, health: "ok" };
  }
  if (event === "sessionend") {
    const reason = String(get(payload, ["reason", "source"], "") || "").trim();
    const summary = reason ? `Session ended (${reason})` : "Session ended";
    const outcome = CANCELLED.has(reason.toLowerCase()) ? "cancelled" : "success";
    const facets = { lifecycle: "ended", outcome, current: { type: "idle", summary }, attention: { level: "none" }, health: "ok", ended: true };
    if (reason) facets.end_reason = reason;
    return facets;
  }
  if (event === "userpromptsubmit") {
    const prompt = truncate(get(payload, ["prompt", "user_prompt"], "") || "", 120);
    return { lifecycle: "active", current: { type: "thinking", summary: prompt || "New prompt", startedAt: nowIso() }, attention: { level: "none" }, health: "ok" };
  }
  if (event === "pretooluse") {
    const tool = String(get(payload, ["tool_name", "toolName"], "tool"));
    if (SUBAGENT_TOOLS.has(tool)) {
      const input = payload.tool_input || payload.toolInput || {};
      const sub = get(input, ["subagent_type", "subagentType", "description"]);
      const name = String(sub || tool);
      return { lifecycle: "active", current: { type: "subagent", name, summary: sub ? `Using ${tool} (${sub})` : `Using ${tool}`, startedAt: nowIso() }, attention: { level: "none" }, health: "ok" };
    }
    return { lifecycle: "active", current: { type: "tool", name: tool, summary: `Using ${tool}`, startedAt: nowIso() }, attention: { level: "none" }, health: "ok" };
  }
  if (event === "posttooluse") {
    const tool = String(get(payload, ["tool_name", "toolName"], "tool"));
    if (SUBAGENT_TOOLS.has(tool)) {
      const resp = payload.tool_response || payload.toolResponse || {};
      const status = String(resp.status || "").toLowerCase();
      if (status === "async_launched" || status === "running" || status === "in_progress") {
        const input = payload.tool_input || payload.toolInput || {};
        const name = String(resp.description || get(input, ["subagent_type", "subagentType", "description"], tool));
        return { lifecycle: "active", current: { type: "subagent", name, summary: `Background: ${name}`, startedAt: nowIso() }, attention: { level: "none" }, health: "ok" };
      }
    }
    return { lifecycle: "active", current: { type: "tool", name: tool, summary: `Finished ${tool}`, startedAt: nowIso() }, attention: { level: "none" }, health: "ok" };
  }
  if (event === "subagentstart") {
    const name = String(get(payload, ["agent_type", "agentType", "description"], "subagent"));
    return { lifecycle: "active", current: { type: "subagent", name, summary: `Subagent: ${name}`, startedAt: nowIso() }, attention: { level: "none" }, health: "ok" };
  }
  if (event === "subagentstop") {
    const name = String(get(payload, ["agent_type", "agentType", "tool_name", "toolName", "description"], "subagent"));
    const bg = bgTasks(payload);
    if (bg.length) return backgroundWork(bg);
    return { lifecycle: "active", current: { type: "thinking", name, summary: `Subagent finished: ${name}`, startedAt: nowIso() }, attention: { level: "none" }, health: "ok" };
  }
  if (event === "posttoolusefailure" || event === "stopfailure") {
    const tool = String(get(payload, ["tool_name", "toolName"], "tool"));
    let summary = `Failed: ${tool}`;
    if (event === "stopfailure" && payload.error) summary = truncate(String(payload.error), 120) || summary;
    return {
      lifecycle: "active",
      current: { type: "tool", name: tool, summary },
      attention: { level: "informational", reason: "failure", title: event === "stopfailure" ? "Turn failed" : `${tool} failed` },
      health: "degraded",
    };
  }
  if (event === "permissionrequest" || event === "permissiondenied") {
    const tool = String(get(payload, ["tool_name", "toolName"], "tool"));
    const dumped = JSON.stringify(get(payload, ["tool_input", "toolInput"], {}));
    return {
      lifecycle: "active",
      current: { type: "waiting", summary: `Permission: ${tool}` },
      attention: { level: "required", reason: "approval", title: "Approval needed in agent", summary: truncate(`${tool}: ${dumped}`, 140) },
      health: "ok",
    };
  }
  if (event === "notification") {
    const ntype = String(get(payload, ["notification_type", "notificationType"], "") || "").trim().toLowerCase().replace(/-/g, "_");
    const title = String(get(payload, ["title", "message"], "") || "");
    let summary = truncate(title, 120) || ntype || "Notification";
    const bg = bgTasks(payload);
    if (ntype === "permission_prompt" || ntype === "permission") {
      return { lifecycle: "active", current: { type: "waiting", summary }, attention: { level: "required", reason: "approval", title: "Approval needed in agent", summary: summary || "Return to the agent to approve" }, health: "ok" };
    }
    if (ntype === "idle_prompt" || ntype === "agent_needs_input" || ntype === "elicitation_dialog") {
      if (bg.length) return backgroundWork(bg);
      if (ntype === "idle_prompt") return null; // Preserve the last real state.
      return yourTurn(summary || "Return to the agent to continue");
    }
    if (["agent_completed", "elicitation_complete", "elicitation_response", "auth_success"].includes(ntype)) {
      return { lifecycle: "active", current: { type: "info", summary }, attention: { level: "none" }, health: "ok" };
    }
    if (bg.length) return backgroundWork(bg);
    const toast = classifyToast(summary);
    if (toast) return toast === "monitor" ? monitorWait(summary, "monitor") : runningBg(summary, "background");
    return { lifecycle: "active", current: { type: "info", summary }, attention: { level: "none" }, health: "ok" };
  }
  if (event === "stop") {
    return stopFacets(payload);
  }
  if (event === "stopcancelled" || event === "elicitation") {
    return yourTurn("Return to the agent to continue");
  }
  if (event === "precompact") {
    const bg = bgTasks(payload);
    if (bg.length) return backgroundWork(bg);
    return { lifecycle: "active", current: { type: "thinking", summary: "Compacting context", startedAt: nowIso() }, attention: { level: "none" }, health: "ok" };
  }
  if (event === "postcompact") {
    const bg = bgTasks(payload);
    return bg.length ? backgroundWork(bg) : { lifecycle: "active", current: { type: "thinking", summary: "Context compacted — continuing" }, attention: { level: "none" }, health: "ok" };
  }
  if (event === "cwdchanged") {
    return { lifecycle: "active", current: { type: "info", summary: "Workspace changed" }, attention: { level: "none" }, health: "ok" };
  }
  if (event === "elicitationresult") {
    const text = truncate(get(payload, ["summary", "title"], "") || "", 120);
    return { lifecycle: "active", current: { type: "info", summary: text || "Elicitation result" }, attention: { level: "none" }, health: "ok" };
  }
  if (event === "taskcreated" || event === "taskcompleted") {
    const label = event === "taskcompleted" ? "Task completed" : "Task created";
    const name = get(payload, ["title", "name"]);
    const text = get(payload, ["title", "name", "description"], "") || "";
    const facets = { lifecycle: "active", current: { type: "info", summary: text ? `${label}: ${text}` : label }, attention: { level: "none" }, health: "ok" };
    if (name) facets.current.name = String(name);
    return facets;
  }
  if (event === "teammateidle") {
    const name = get(payload, ["name", "agent_type", "agentType", "description"]);
    const facets = { lifecycle: "active", current: { type: "info", summary: name ? `Teammate idle: ${name}` : "Teammate idle" }, attention: { level: "none" }, health: "ok" };
    if (name) facets.current.name = String(name);
    return facets;
  }
  return null;
}

// Events the hosts register that must never paint a facet. Fire-and-drop —
// a facet here would invent status from events that carry none (invariant 3).
const SILENT_NOOP = new Set([
  "setup", "userpromptexpansion", "posttoolbatch", "messagedisplay",
  "instructionsloaded", "configchange", "directoryadded", "filechanged",
]);

function detectIde() {
  const term = (process.env.TERM_PROGRAM || "").toLowerCase();
  if (process.env.CURSOR_TRACE_ID || process.env.CURSOR_AGENT || term.includes("cursor")) return "cursor";
  if (process.env.VSCODE_INJECTION || process.env.VSCODE_PID || process.env.VSCODE_GIT_IPC_HANDLE || term === "vscode" || term === "vscode-insiders") return "vscode";
  return null;
}

// Short human label for the hosting terminal/tab when known. Same map as
// `nerve.py` `_terminal_label()` — the focus breadcrumb should say where the
// session lives, not the literal "session".
const TERMINAL_LABELS = [
  ["ITERM_SESSION_ID", "iTerm"],
  ["TERM_SESSION_ID", "Terminal"],
  ["WEZTERM_PANE", "WezTerm"],
  ["KITTY_WINDOW_ID", "Kitty"],
  ["TMUX_PANE", "tmux"],
];

function terminalLabel() {
  for (const [key, label] of TERMINAL_LABELS) {
    if (process.env[key]) return label;
  }
  return process.env.TERM_PROGRAM || null;
}

// Wire paths. A cwd is written in this machine's rules, and the hub and every
// surface may read it from another one, so shape is asked of the string rather
// than of the host. Same rules as `nerve_platform::path` on the Rust side,
// which is what decodes what we emit. The POSIX encoding is unchanged: it is
// the contract four decoders already agree on.
const DRIVE_ROOTED = /^[A-Za-z]:[\\/]/;

function pathStyle(p) {
  if (!p) return null;
  if (p.startsWith("/")) return "posix";
  if (p.startsWith("\\\\")) return "windows";
  if (DRIVE_ROOTED.test(p)) return "windows";
  return null;
}

function encodePath(p) {
  return encodeURI(p).replace(/#/g, "%23");
}

function fileUri(p) {
  const style = pathStyle(p);
  if (!style) return null;
  if (style === "posix") return "file://" + encodePath(p);
  const slashed = p.replace(/\\/g, "/");
  // UNC: the server is the URL's authority (RFC 8089).
  if (slashed.startsWith("//")) return "file://" + encodePath(slashed.slice(2));
  return "file:///" + encodePath(slashed);
}

// `vscode://file/C:/work` — the drive needs the URL's leading slash, which a
// POSIX path already supplies itself.
function ideFilePath(cwd) {
  if (pathStyle(cwd) === "posix") return cwd;
  const slashed = cwd.replace(/\\/g, "/");
  return slashed.startsWith("/") ? slashed : "/" + slashed;
}

function buildLocation(payload, cwd) {
  const project = path.basename(cwd || "") || "unknown";
  const host = terminalLabel() || "session";
  let focusHint = `Claude Code · ${project} · ${host}`;
  if (cwd) focusHint += ` · ${cwd}`;
  const scheme = detectIde();
  const openURL =
    scheme && pathStyle(cwd) ? `${scheme}://file${ideFilePath(cwd)}` : fileUri(cwd);
  const loc = { openURL, focusHint, logPath: get(payload, ["transcript_path", "transcriptPath"]) };
  return Object.fromEntries(Object.entries(loc).filter(([, v]) => v != null && v !== ""));
}

// Display-only actions: Open/Focus first, then Copy, then Open logs.
// Never approve/submit — Nerve does not reverse-control (invariant 6).
function localActions(location) {
  const actions = [];
  const hasUrl = Boolean(location && location.openURL);
  const hasHint = Boolean(location && location.focusHint);
  if (hasUrl || hasHint) {
    actions.push({
      id: "open",
      title: hasUrl ? "Open" : "Focus",
      kind: hasUrl ? "open" : "focus",
      state: "available",
      destructive: false,
      confirmationRequired: false,
    });
  }
  actions.push({
    id: "copy",
    title: "Copy",
    kind: "copy_summary",
    state: "available",
    destructive: false,
    confirmationRequired: false,
  });
  // macOS ActionService already reveals `location.logPath` on this action id.
  if (location && location.logPath) {
    actions.push({
      id: "open_logs",
      title: "Open logs",
      kind: "open_logs",
      state: "available",
      destructive: false,
      confirmationRequired: false,
    });
  }
  return actions;
}

function buildJob(payload, session, facets) {
  const cwd = cwdOf(payload);
  const project = path.basename(cwd || "") || "unknown";
  const now = nowIso();
  const meta = META.claude;
  const location = buildLocation(payload, cwd);
  const extensions = {
    hookEvent: get(payload, ["hook_event_name", "hookEventName", "event"], ""),
    sessionId: session,
    model: get(payload, ["model"]),
    slot: slotId(payload),
  };
  const pid = agentPid();
  if (pid) extensions.pid = pid;
  const at = get(payload, ["agent_type", "agentType"]);
  if (at) extensions.agentType = at;
  const ev = eventName(payload);
  if ((ev === "subagentstart" || ev === "subagentstop") && get(payload, ["agent_id", "agentId"])) {
    extensions.agentId = String(get(payload, ["agent_id", "agentId"]));
  }
  if (facets.end_reason) extensions.endReason = facets.end_reason;
  if (ev === "userpromptsubmit") {
    const prompt = truncate(get(payload, ["prompt", "user_prompt"], "") || "", PROMPT_MAX);
    if (prompt) {
      extensions.lastPrompt = prompt;
      extensions.lastPromptAt = now;
    }
  }
  const job = {
    id: `${meta.id}:${session}`,
    kind: "session",
    name: project,
    alias: machineAlias(),
    lifecycle: facets.lifecycle,
    current: facets.current,
    attention: facets.attention || { level: "none" },
    health: facets.health || "ok",
    progress: { kind: "none" },
    producer: meta,
    context: { project, workspace: cwd, labels: ["claude", "session"] },
    location,
    capabilities: [],
    actions: localActions(location),
    createdAt: now,
    startedAt: now,
    updatedAt: now,
    version: versionMs(),
    extensions: Object.fromEntries(Object.entries(extensions).filter(([, v]) => v != null && v !== "")),
  };
  // current.startedAt is present on every non-ended job (decision 7) — the hub
  // already does this, and a missing stamp is three silent schemas.
  if (!facets.ended && job.current && !job.current.startedAt) job.current.startedAt = now;
  if (facets.outcome) job.outcome = facets.outcome;
  if (facets.ended) {
    job.endedAt = now;
    job.lifecycle = "ended";
  }
  return job;
}

// Which terminal window this session belongs to, so a new session in the same
// window supersedes the old one instead of stacking up.
//
// `WT_SESSION` (Windows Terminal) and `ConEmuPID` are here because without
// them every Windows session fell through to the pid token, and two concurrent
// sessions under one agent host collided on a single slot.
const SLOT_ENV_KEYS = [
  "TERM_SESSION_ID",
  "ITERM_SESSION_ID",
  "WEZTERM_PANE",
  "KITTY_WINDOW_ID",
  "TMUX_PANE",
  "WT_SESSION",
  "ConEmuPID",
];

function slotId(payload) {
  let token = "default";
  for (const key of SLOT_ENV_KEYS) {
    const value = process.env[key];
    if (value) { token = `${key}:${value}`; break; }
  }
  if (token === "default") {
    const pid = agentPid();
    if (pid) token = `pid:${pid}`;
  }
  return crypto.createHash("sha256").update(`claude\0${machineAlias()}\0${token}`).digest("hex").slice(0, 32);
}

// The agent process, as best this hook can tell.
//
// Answered once per hook run: `slotId` and the job body both want it, and on
// POSIX each answer costs up to six `ps` spawns.
let _agentPid;
let _agentPidResolved = false;
function agentPid() {
  if (_agentPidResolved) return _agentPid;
  _agentPidResolved = true;
  const cached = identityLoad();
  if (cached && cached.pid !== undefined) {
    _agentPid = cached.pid > 1 ? cached.pid : null;
    return _agentPid;
  }
  _agentPid = process.platform === "win32" ? windowsAgentPid() : posixAgentPid();
  identitySave(machineAlias(), _agentPid);
  return _agentPid;
}

// Windows has no `ps`, and the equivalent — `Get-CimInstance Win32_Process` —
// costs roughly half a second to start. This hook runs on every event and must
// never be what makes an agent feel slow, so it does not climb at all.
//
// It does not need to: `hooks.json` invokes us in exec form (`node` + args,
// no shell), so the parent already *is* the agent host. The POSIX climb below
// exists to see past a shell wrapper, which is not in the way here. If this is
// ever wrong the cost is a row that leaves on SessionEnd instead of on reap —
// the same fail-open posture as every other arm of this file.
function windowsAgentPid() {
  const pid = process.ppid;
  return pid > 1 ? pid : null;
}

function posixAgentPid() {
  let pid = process.ppid;
  let last = pid;
  for (let i = 0; i < 6 && pid && pid > 1; i++) {
    const r = spawnSync("ps", ["-p", String(pid), "-o", "ppid=,comm="], {
      encoding: "utf8",
      timeout: 400,
      stdio: ["ignore", "pipe", "ignore"],
    });
    const line = (r.stdout || "").trim();
    const parts = line.split(/\s+/, 2);
    const ppid = parseInt(parts[0], 10);
    const name = path.basename(parts[1] || "").toLowerCase();
    const shells = new Set(["bash", "sh", "zsh", "fish", "dash", "node"]);
    if (name && !shells.has(name) && !name.endsWith(".sh")) return pid;
    last = pid;
    if (!ppid || ppid === pid) return last;
    pid = ppid;
  }
  return last > 1 ? last : null;
}

let _stateDir;
function stateDir() {
  // mkdir once per process: slotRead/Write/Clear each used to pay it.
  if (_stateDir) return _stateDir;
  const d = path.join(os.tmpdir(), "nerve-hook");
  try { fs.mkdirSync(d, { recursive: true, mode: 0o700 }); } catch (_) {}
  _stateDir = d;
  return d;
}

// ── Identity cache ─────────────────────────────────────────────────────────
//
// One process per hook event means the in-process memo for `machineAlias()`
// (a `scutil` spawn) and `agentPid()` (up to six `ps` spawns) dies with the
// process. They are keyed by the *terminal token*, which is free to compute
// (a handful of env lookups) — the full `slotId` hash is not, because it folds
// the alias and pid in. First event per UI slot pays the climb; every later one
// reads a small JSON file. SessionStart invalidates so a renamed machine or a
// restarted host is picked up on the next conversation.
const IDENTITY_TTL_MS = 24 * 60 * 60 * 1000;

function terminalToken() {
  for (const key of SLOT_ENV_KEYS) {
    const value = process.env[key];
    if (value) return `${key}:${value}`;
  }
  return "default";
}

function identityPath() {
  const token = terminalToken();
  return path.join(stateDir(), `identity-${token.replace(/[^\w.-]/g, "_")}.json`);
}

function identityLoad() {
  try {
    const data = JSON.parse(fs.readFileSync(identityPath(), "utf8"));
    if (!data || typeof data !== "object") return null;
    const ts = Date.parse(data.ts || "");
    if (!Number.isFinite(ts) || Date.now() - ts > IDENTITY_TTL_MS) return null;
    return data;
  } catch (_) {
    return null;
  }
}

function identitySave(alias, pid) {
  try {
    fs.writeFileSync(
      identityPath(),
      JSON.stringify({ alias, pid: pid ?? null, ts: new Date().toISOString() }),
    );
  } catch (_) {}
}

function identityInvalidate() {
  try { fs.unlinkSync(identityPath()); } catch (_) {}
}

function slotRead(id) {
  try {
    const data = JSON.parse(fs.readFileSync(path.join(stateDir(), `${id}.json`), "utf8"));
    return data.session_id ? String(data.session_id) : null;
  } catch (_) {
    return null;
  }
}

function slotWrite(id, session) {
  try {
    fs.writeFileSync(path.join(stateDir(), `${id}.json`), JSON.stringify({ session_id: session, ts: nowIso() }));
  } catch (_) {}
}

function slotClear(id, session) {
  try {
    if (session && slotRead(id) !== session) return;
    fs.unlinkSync(path.join(stateDir(), `${id}.json`));
  } catch (_) {}
}

// Node's own HTTP client, not a hand-written request over a raw socket.
// Framing and `Content-Length` are the runtime's job.
//
// Fire-and-forget (CLAUDE.md invariant 2): the promise resolves once the
// request bytes are flushed, or the connect fails, whichever comes first.
// The hub's status line is never read. A closed port, a refusal and a hub
// that accepts and then sits there all look the same to the agent: this
// returns, and the process exits 0. `port` is for tests only — production
// never passes it, because the ingest address is fixed and a test must never
// bind 17890.
function postIngest(port, urlPath, body) {
  const payload = Buffer.isBuffer(body) ? body : Buffer.from(body);
  return new Promise((resolve) => {
    let done = false;
    const finish = () => {
      if (done) return;
      done = true;
      resolve();
    };
    let request;
    try {
      request = http.request({
        host: INGEST_HOST,
        port,
        method: "POST",
        path: urlPath,
        timeout: INGEST_TIMEOUT_MS,
        headers: {
          "User-Agent": "nerve-hook-node/0.1",
          "Content-Type": "application/json",
          "Content-Length": payload.length,
          Connection: "close",
        },
      });
    } catch (_) {
      finish();
      return;
    }
    request.on("timeout", () => {
      request.destroy();
      finish();
    });
    request.on("error", finish);
    // If a response shows up anyway, drop it. Waiting for it is the bug.
    request.on("response", (response) => {
      response.resume();
      if (response.socket) response.socket.unref();
    });
    request.end(payload, () => {
      if (request.socket) request.socket.unref();
      finish();
    });
  });
}

// Accepts one job or several — a SessionStart that supersedes a ghost closes
// the old conversation and opens the new one in one round trip. The envelope
// was always `{ jobs: [...] }`; `JobStore::apply_snapshot` already takes a Vec.
function postSnapshot(alias, kind, jobs, port = INGEST_PORT) {
  const list = Array.isArray(jobs) ? jobs : [jobs];
  const body = Buffer.from(JSON.stringify({ alias, machineKind: kind, jobs: list }));
  return postIngest(port, "/v1/snapshot", body);
}

async function processEvent(payload) {
  const event = eventName(payload);
  if (!event) return null;
  // Silent-noop events exit before alias / slot / ps / HTTP / disk (Phase 4).
  if (SILENT_NOOP.has(event)) return null;
  const allowed = new Set([
    "sessionstart", "setup", "sessionend", "userpromptsubmit", "userpromptexpansion",
    "pretooluse", "posttooluse", "posttoolusefailure", "posttoolbatch",
    "permissionrequest", "permissiondenied", "notification", "messagedisplay",
    "stop", "stopfailure", "stopcancelled", "subagentstart", "subagentstop",
    "taskcreated", "taskcompleted", "teammateidle", "instructionsloaded",
    "configchange", "cwdchanged", "directoryadded", "filechanged",
    "precompact", "postcompact", "elicitation", "elicitationresult",
  ]);
  if (!allowed.has(event)) return null;
  if (get(payload, ["agent_id", "agentId"]) && SUBAGENT_NOISE.has(event)) return null;
  const facets = mapEvent(event, payload);
  if (!facets) return null;
  const session = sessionId(payload);
  // A new conversation is the moment a stale identity is most wrong (renamed
  // machine, restarted host) — drop the cache so the climb runs once here.
  if (event === "sessionstart") identityInvalidate();
  const slot = slotId(payload);
  const pending = [];
  if (event === "sessionstart") {
    const prev = slotRead(slot);
    if (prev && prev !== session) {
      const ended = mapEvent("sessionend", { reason: "superseded" });
      ended.end_reason = "superseded";
      const ghost = buildJob(payload, prev, ended);
      ghost.extensions.endReason = "superseded";
      if (String(ghost.id).split(":").length - 1 < 2) pending.push(ghost);
    }
  }
  const job = buildJob(payload, session, facets);
  if (String(job.id).split(":").length - 1 >= 2) return null;
  pending.push(job);
  // One POST carries the ghost and the new row: two conversations, not two
  // jobs per conversation (invariant 1).
  await postSnapshot(job.alias, machineKind(), pending);
  if (event === "sessionend") slotClear(slot, session);
  else slotWrite(slot, session);
  return job;
}

/**
 * Test-only adapter: `node nerve.js --map <fixture.json>` prints raw facets or
 * `null`. Not an env flag and not `NERVE_*` (invariant 2); production hooks
 * never pass argv. Exits 0 even on a bad path so a harness can never wedge.
 */
function mapFixture(path) {
  let out = "null";
  try {
    const payload = JSON.parse(fs.readFileSync(path, "utf8"));
    if (payload && typeof payload === "object") {
      out = JSON.stringify(mapEvent(eventName(payload), payload) ?? null);
    }
  } catch (_) {
    out = "null";
  }
  try { process.stdout.write(out + "\n"); } catch (_) {}
  return 0;
}

async function main() {
  if (process.argv[2] === "--map" && process.argv[3]) return mapFixture(process.argv[3]);
  let raw = "";
  try { raw = fs.readFileSync(0, "utf8"); } catch (_) { return 0; }
  if (!raw.trim()) return 0;
  let payload;
  try { payload = JSON.parse(raw); } catch (_) { return 0; }
  if (!payload || typeof payload !== "object") return 0;
  try { await processEvent(payload); } catch (_) {}
  return 0;
}

if (require.main === module) {
  main().then((code) => process.exit(code));
}

module.exports = {
  processEvent, mapEvent, eventName, sessionId,
  fileUri, pathStyle, buildLocation, postIngest, postSnapshot, mapFixture,
  agentPid, slotId, machineAlias, machineKind, localActions,
};
