#!/usr/bin/env python3
"""Nerve hook — Codex official command hook (python3 ${PLUGIN_ROOT}/hooks/nerve.py).

Reads one JSON hook event from stdin, maps it to a single **main-session** Job
snapshot (id = {producer}:{session_id}), and POSTs to the fixed local ingest
URL. Always exit 0 (observability must never block the agent).

Claude Code uses hooks/nerve.js (Node exec form). Grok uses type: http.

Design (one job per conversation)
---------------------------------
- Only the main session is a Nerve job. Subagents are **not** separate rows.
- Job id is **always** ``{producer}:{session_id}`` — never append agent_id.
- SubagentStart / SubagentStop / background_tasks only refine the main session's
  ``current`` facet (Running vs Attention).
- Hooks that fire *inside* a subagent (payload has agent_id) are ignored for
  tool chatter (Pre/PostToolUse…), so the panel is not flooded. Permission and
  lifecycle brackets still update the main session when they matter.

Orthogonal event → facet map (no free-text status inference)
------------------------------------------------------------
+----------------------+------------------+------------------+---------------+
| Event                | lifecycle        | current.type     | attention     |
+----------------------+------------------+------------------+---------------+
| SessionStart         | active           | starting         | none          |
| UserPromptSubmit     | active           | thinking         | none          |
| Pre/PostToolUse      | active           | tool | subagent  | none          |
|   (main thread)      |                  |                  |               |
| Pre/PostToolUse      | (skip — no POST) |                  |               |
|   (agent_id set)     |                  |                  |               |
| SubagentStart        | active           | subagent         | none          |
| SubagentStop + bg    | active           | subagent         | none          |
| SubagentStop no bg   | active           | thinking         | none          |
| Stop + shell/subagent bg | active      | subagent         | none (Running)|
| Stop + monitor-only bg   | active      | monitor + partial| none (Monitor)|
| Stop empty bg        | active           | completed        | none          |
| idle_prompt (no bg)  | active           | idle             | input         |
| idle_prompt + shell/agent toast | active | subagent     | none (Running)|
| idle_prompt + monitor toast     | active | monitor      | none (Monitor)|
| Permission*          | active           | waiting          | approval      |
| PreCompact           | active           | thinking | bg     | none (Running / Monitor) |
| PostCompact + bg     | active           | subagent | monitor | none          |
| PostCompact no bg    | active           | idle             | input         |
| SessionEnd           | ended + success  | idle             | none          |
+----------------------+------------------+------------------+---------------+
Stop / idle ≠ leave panel. Only SessionEnd (or a superseding SessionStart
from the same UI slot after /new · /clear · fork) removes the row.

Hosts sometimes skip SessionEnd when the user starts a fresh conversation in
the same terminal/process. The hook keeps a tiny temp-dir slot map so the
previous session_id is closed when a new SessionStart arrives for that slot.

No NERVE_* config env. Alias is a free-form machine label (prefer stable
Bonjour LocalHostName on macOS). Nerve shows whatever alias arrives — no
allow-list.

Install (GitHub marketplace — repo root)
----------------------------------------
  Claude:  /plugin marketplace add Roy-Kid/nerve
           /plugin install nerve@nerve
  Codex:   codex plugin marketplace add Roy-Kid/nerve
           codex plugin add nerve@nerve
"""

from __future__ import annotations

import functools
import hashlib
import json
import os
import platform
import re
import socket
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

# Fixed ingest — tunnels (SSH RemoteForward) make remotes look like loopback.
INGEST_HOST = "127.0.0.1"
INGEST_PORT = 17890
INGEST_BASE = f"http://{INGEST_HOST}:{INGEST_PORT}"

#: How long connect + write may take. The hook never reads the response, so
#: this does not include the hub's answer. Fail-open: past this the hook gives
#: up on the hub, never on the agent.
INGEST_TIMEOUT = 0.25

PRODUCER_META: dict[str, dict[str, str]] = {
    "claude": {"id": "claude-code", "name": "Claude Code", "kind": "agent.claude"},
    "grok": {"id": "grok", "name": "Grok", "kind": "agent.grok"},
    "codex": {"id": "codex", "name": "Codex", "kind": "agent.codex"},
}

# High-frequency tool events that fire inside subagents — skip so the main
# session row stays a clean lifecycle, not a mirror of every child tool call.
_SUBAGENT_INTERNAL_NOISE = frozenset(
    {
        "pretooluse",
        "posttooluse",
        "posttoolusefailure",
        "userpromptsubmit",
    }
)

#: Tools that mean the main session is driving a subagent (pre- and post-use).
_SUBAGENT_TOOLS = frozenset(
    {
        "spawn_subagent",
        "get_command_or_subagent_output",
        "Task",
        "Agent",
    }
)

#: Events the hosts register that must never paint a facet. Fire-and-drop —
#: a facet here would invent status from events that carry none (invariant 3).
_SILENT_NOOP = frozenset(
    {
        "setup",
        "userpromptexpansion",
        "posttoolbatch",
        "messagedisplay",
        "instructionsloaded",
        "configchange",
        "directoryadded",
        "filechanged",
    }
)

# Ephemeral per-slot memory (temp dir only). Hosts often fire SessionStart for
# /new · /clear · fork without SessionEnd for the previous conversation id.
# We remember the last session_id per UI/process slot and emit an ended job for
# the previous id when a new one starts. Tests may override this path.
_STATE_DIR_OVERRIDE: Path | None = None


def _now_iso() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _version_ms() -> int:
    return int(time.time() * 1000)


@functools.lru_cache(maxsize=1)
def _machine_alias() -> str:
    """Prefer stable Bonjour LocalHostName on macOS; fall back to gethostname short.

    Campus / DHCP networks often rewrite DNS hostnames (e.g. RoydeAir.kemi…,
    emp-181-64.eduroam…), while Nerve Settings "This Mac" uses the Bonjour
    LocalHostName (e.g. RoydeMacBook-Air). Mismatch → 403 and silent drop.

    Cached in-process *and* on disk (see `_identity_load`): one process is one
    event, so a per-process memo alone would pay the `scutil` spawn every time.
    """
    cached = _identity_load()
    if cached and cached.get("alias"):
        return str(cached["alias"])
    alias = None
    if sys.platform == "darwin":
        try:
            import subprocess

            out = subprocess.check_output(
                ["/usr/sbin/scutil", "--get", "LocalHostName"],
                stderr=subprocess.DEVNULL,
                text=True,
                timeout=1.0,
            ).strip()
            if out:
                alias = out
        except Exception:
            pass
    if alias is None:
        host = socket.gethostname() or "local"
        alias = host.split(".")[0].strip() or host
    _identity_save(alias, None)
    return alias


@functools.lru_cache(maxsize=1)
def _machine_kind() -> str:
    """Which OS family reported this job. Constant per process — see [_machine_alias]."""
    system = platform.system().lower()
    if system == "darwin":
        return "darwin"
    if system == "linux":
        # crude WSL check without env
        try:
            release = platform.release().lower()
            if "microsoft" in release or "wsl" in release:
                return "wsl"
        except Exception:
            pass
        return "linux"
    if system == "windows":
        return "windows"
    return "unknown"


def _get(d: dict[str, Any], *keys: str, default: Any = None) -> Any:
    for k in keys:
        if k in d and d[k] is not None and d[k] != "":
            return d[k]
    return default


def _event_name(payload: dict[str, Any]) -> str:
    raw = _get(
        payload,
        "hook_event_name",
        "hookEventName",
        "event",
        "event_name",
        default="",
    )
    s = str(raw).strip()
    out = []
    for ch in s:
        if ch in ("_", "-", " "):
            continue
        out.append(ch.lower())
    return "".join(out)


def _detect_producer(payload: dict[str, Any]) -> str:
    """Detect producer from host env (plugin runner) then payload heuristics."""
    # Host harness injects these for plugin hooks — most reliable signal.
    if os.environ.get("GROK_PLUGIN_ROOT") or os.environ.get("GROK_SESSION_ID") or os.environ.get(
        "GROK_HOOK_EVENT"
    ):
        return "grok"
    if os.environ.get("CODEX_HOME") or os.environ.get("PLUGIN_ROOT"):
        # Codex plugin runner sets PLUGIN_ROOT; Claude uses CLAUDE_PLUGIN_ROOT only.
        # Prefer explicit codex markers when present.
        if os.environ.get("CODEX_HOME") or "codex" in (
            os.environ.get("PLUGIN_ROOT") or ""
        ).lower():
            return "codex"

    model = str(_get(payload, "model", default="") or "").lower()
    if "grok" in model:
        return "grok"
    if "gpt" in model or "o3" in model or "o4" in model or "codex" in model:
        return "codex"

    # Schema heuristics
    if payload.get("cursor_version"):
        return "claude"

    session_source = str(_get(payload, "source", default="") or "").lower()
    if "codex" in session_source:
        return "codex"
    if "grok" in session_source:
        return "grok"

    # Claude plugin runner always sets CLAUDE_PLUGIN_ROOT (Grok also sets it as an
    # alias, but GROK_* was already checked above).
    if os.environ.get("CLAUDE_PLUGIN_ROOT") and not os.environ.get("GROK_PLUGIN_ROOT"):
        return "claude"

    return "claude"


def _session_id(payload: dict[str, Any]) -> str:
    sid = _get(
        payload,
        "session_id",
        "sessionId",
        "conversation_id",
        "conversationId",
        "thread_id",
        "threadId",
        default=None,
    )
    if sid:
        return str(sid)
    cwd = _get(payload, "cwd", "workspace_root", "workspaceRoot", default="")
    return f"anon-{abs(hash(str(cwd))) % 10_000_000}"


def _agent_id(payload: dict[str, Any]) -> str | None:
    """Present when the hook fires inside a subagent (Claude Code)."""
    raw = _get(payload, "agent_id", "agentId", default=None)
    if raw is None:
        return None
    s = str(raw).strip()
    return s or None


def _agent_type(payload: dict[str, Any]) -> str | None:
    raw = _get(payload, "agent_type", "agentType", default=None)
    if raw is None:
        return None
    s = str(raw).strip()
    return s or None


def _cwd(payload: dict[str, Any]) -> str:
    cwd = _get(payload, "cwd", "working_directory", default=None)
    if cwd:
        return str(cwd)
    roots = payload.get("workspace_roots") or payload.get("workspaceRoots") or []
    if isinstance(roots, list) and roots:
        return str(roots[0])
    return str(Path.cwd())


def _project_name(cwd: str) -> str:
    if not cwd:
        return "unknown"
    return Path(cwd).name or "unknown"


#: How much of a user prompt a surface may show. Longer than the one-line
#: `current.summary` cap: the sidebar's Prompt panel wraps and scrolls it.
PROMPT_MAX = 400


def _truncate(s: str | None, n: int = 160) -> str | None:
    if not s:
        return None
    s = " ".join(str(s).split())
    if len(s) <= n:
        return s
    return s[: n - 1] + "…"


def _notification_type(payload: dict[str, Any]) -> str:
    """Structured notification kind from the harness (never parse free-text message)."""
    raw = _get(
        payload,
        "notification_type",
        "notificationType",
        default="",
    )
    return str(raw or "").strip().lower().replace("-", "_")


def _background_tasks(payload: dict[str, Any]) -> list[Any]:
    """In-flight background work from Stop / SubagentStop (Claude Code ≥ 2.1.145).

    Empty or missing ⇒ none. Non-empty ⇒ main session is paused on background
    agents/shells, not waiting for the human. Prefer this over free-text.
    """
    raw = payload.get("background_tasks")
    if raw is None:
        raw = payload.get("backgroundTasks")
    return raw if isinstance(raw, list) else []


def _normalize_toast(text: str) -> str:
    return " ".join(str(text or "").lower().split())


def _is_harness_background_wait_toast(text: str) -> bool:
    """True for harness toasts that mean *bg work still in flight*, not human idle.

    Preferred signal is non-empty ``background_tasks``. Some Notification paths
    (often ``idle_prompt``) still emit only the UI string, e.g.
    ``Waiting for 1 background agent to finish``, ``1 shell/monitor still running``.
    Those must never become Attention.
    """
    return _classify_bg_wait_toast(text) is not None


def _classify_bg_wait_toast(text: str) -> str | None:
    """Classify known bg-wait toasts → ``\"running\"`` | ``\"monitor\"`` | None.

    - shell / subagent / agent still in flight → Running (blue)
    - monitor-only waiting for feedback → Monitor (purple, watching the stream)
    - shell/monitor combined → Running (shell is active work)
    Unknown free text → None (do not invent Attention from arbitrary strings).
    """
    s = _normalize_toast(text)
    if not s:
        return None

    has_shell = "shell" in s
    has_monitor = "monitor" in s
    has_agent = "agent" in s
    still = "still running" in s or "still run" in s
    waiting = "waiting" in s or "finish" in s or "running" in s

    # "1 shell/monitor still running", "2 shells still running"
    if still and (has_shell or has_monitor):
        if has_monitor and not has_shell and not has_agent:
            return "monitor"
        return "running"

    # "Waiting for N background agent(s) to finish" / "Waiting for N agents"
    if has_agent and waiting and (
        "background" in s or "waiting for" in s or "finish" in s
    ):
        return "running"
    if "waiting for" in s and "background" in s and "task" in s:
        return "running"

    # "Waiting for monitor" / pure monitor feedback wait
    if has_monitor and waiting and not has_shell and not has_agent:
        return "monitor"
    if has_shell and waiting:
        return "running"

    return None


def _task_work_kind(task: Any) -> str:
    """Map one background_tasks entry → ``monitor`` | ``shell`` | ``subagent``."""
    if not isinstance(task, dict):
        return "subagent"
    raw = str(task.get("type") or task.get("kind") or "").strip().lower()
    if not raw:
        # Fall back on description hints when type is missing.
        desc = str(
            task.get("description")
            or task.get("name")
            or task.get("agent_type")
            or task.get("agentType")
            or ""
        ).lower()
        if "monitor" in desc:
            return "monitor"
        if "shell" in desc or "bash" in desc:
            return "shell"
        return "subagent"
    if "monitor" in raw:
        return "monitor"
    if raw in ("shell", "bash", "command", "local_shell", "powershell") or "shell" in raw:
        return "shell"
    return "subagent"


def _running_background_facets(summary: str, *, name: str = "background") -> dict[str, Any]:
    """Main session still working (bg agents/shells) → Running, not Attention."""
    return {
        "lifecycle": "active",
        "current": {
            "type": "subagent",
            "name": name,
            "summary": summary,
            "startedAt": _now_iso(),
        },
        "attention": {"level": "none"},
        "health": "ok",
    }


def _monitor_wait_facets(summary: str, *, name: str = "monitor") -> dict[str, Any]:
    """Monitor waiting for feedback — partial phase done → Monitor (purple).

    Not Attention: nothing needs the human yet; a background monitor is open.
    Not Running: main turn is idle while the monitor holds the stream.
    """
    return {
        "lifecycle": "active",
        "outcome": "partial",
        "current": {
            "type": "monitor",
            "name": name,
            "summary": summary,
            "startedAt": _now_iso(),
        },
        "attention": {"level": "none"},
        "health": "ok",
    }


def _facets_for_bg_wait_toast(summary: str) -> dict[str, Any]:
    """Map a known harness bg-wait toast string → Running or Monitor facets."""
    kind = _classify_bg_wait_toast(summary)
    if kind == "monitor":
        return _monitor_wait_facets(summary)
    return _running_background_facets(summary)


def _bg_kind_label(kinds: set[str]) -> str:
    """Compact kind label for the row: one work kind, or ``mixed`` when they differ."""
    uniq = sorted(kinds)
    if not uniq:
        return "subagent"
    if len(uniq) > 1:
        return "mixed"
    return uniq[0] if uniq[0] in ("monitor", "shell") else "subagent"


def _background_work_facets(payload: dict[str, Any], tasks: list[Any]) -> dict[str, Any]:
    """Structured facets while background_tasks is non-empty.

    - Any shell / subagent still running → Running (blue)
    - Monitor-only → Monitor purple (watching the stream, not executing)
    """
    n = len(tasks)
    kinds = {_task_work_kind(t) for t in tasks}
    name = _bg_kind_label(kinds)
    # List every task, not just the first: one summary is all a row has.
    descs: list[str] = []
    for t in tasks:
        if not isinstance(t, dict):
            continue
        d = (
            t.get("description")
            or t.get("name")
            or t.get("agent_type")
            or t.get("agentType")
            or t.get("type")
            or t.get("kind")
            or ""
        )
        if d:
            cut = _truncate(str(d), 40)
            if cut:
                descs.append(cut)
    summary = f"{n} background task(s)"
    if descs:
        summary = f"{summary}: {_truncate(', '.join(descs), 120)}"

    # Pure monitor queue → Monitor (purple); mix with shell/subagent → Running.
    if kinds and kinds <= {"monitor"}:
        return _monitor_wait_facets(summary, name=name)
    return _running_background_facets(summary, name=name)


def _your_turn(attention_summary: str) -> dict[str, Any]:
    return {
        "lifecycle": "active",
        "current": {
            "type": "idle",
            "summary": "Your turn — continue in the agent UI",
        },
        "attention": {
            "level": "suggested",
            "reason": "input",
            "title": "Your turn in agent",
            "summary": attention_summary,
        },
        "health": "ok",
    }


def _map_event(event: str, payload: dict[str, Any]) -> dict[str, Any] | None:
    """Map hook **event type** (+ structured fields) → main-session job facets.

    Status is never inferred from free-text titles/messages. Controlled vocabulary:
      current.type: thinking | tool | subagent | monitor | waiting | idle | starting | info
      attention.reason: input | approval | failure | (none)
    """
    if event == "sessionstart":
        # Open but not working yet — first UserPromptSubmit is when work starts.
        # "starting" (Ready) ≠ idle your_turn: nothing is in flight, not waiting on you.
        return {
            "lifecycle": "active",
            "current": {
                "type": "starting",
                "summary": "Ready",
                "startedAt": _now_iso(),
            },
            "attention": {"level": "none"},
            "health": "ok",
        }

    if event == "sessionend":
        reason = str(_get(payload, "reason", "source", default="") or "").strip()
        summary = "Session ended"
        if reason:
            summary = f"Session ended ({reason})"
        # Intentional leave vs clean completion — both leave the panel; reason is for logs.
        cancelled = {
            "clear",
            "logout",
            "prompt_input_exit",
            "bypass_permissions_disabled",
            "resume",
            "superseded",
            "process_gone",
            "aborted",
            "dismissed",
        }
        outcome = "cancelled" if reason.lower() in cancelled else "success"
        facets: dict[str, Any] = {
            "lifecycle": "ended",
            "outcome": outcome,
            "current": {"type": "idle", "summary": summary},
            "attention": {"level": "none"},
            "health": "ok",
            "ended": True,
        }
        if reason:
            facets["end_reason"] = reason
        return facets

    if event == "userpromptsubmit":
        prompt = _truncate(_get(payload, "prompt", "user_prompt", default=""), 120)
        return {
            "lifecycle": "active",
            "current": {
                "type": "thinking",
                "summary": prompt or "New prompt",
                "startedAt": _now_iso(),
            },
            "attention": {"level": "none"},
            "health": "ok",
        }

    if event == "pretooluse":
        tool = str(_get(payload, "tool_name", "toolName", default="tool"))
        # Main agent spawning / waiting on a subagent tool → Running (subagent).
        if tool in _SUBAGENT_TOOLS:
            tool_input = payload.get("tool_input") or payload.get("toolInput") or {}
            if not isinstance(tool_input, dict):
                tool_input = {}
            sub_type = _get(
                tool_input,
                "subagent_type",
                "subagentType",
                "description",
                default=None,
            )
            name = str(sub_type or tool)
            summary = f"Using {tool}"
            if sub_type:
                summary = f"Using {tool} ({sub_type})"
            return {
                "lifecycle": "active",
                "current": {
                    "type": "subagent",
                    "name": name,
                    "summary": summary,
                    "startedAt": _now_iso(),
                },
                "attention": {"level": "none"},
                "health": "ok",
            }
        return {
            "lifecycle": "active",
            "current": {
                "type": "tool",
                "name": tool,
                "summary": f"Using {tool}",
                "startedAt": _now_iso(),
            },
            "attention": {"level": "none"},
            "health": "ok",
        }

    if event == "posttooluse":
        tool = str(_get(payload, "tool_name", "toolName", default="tool"))
        # Background Agent launch returns early; keep main session as subagent work.
        if tool in _SUBAGENT_TOOLS:
            tool_response = payload.get("tool_response") or payload.get("toolResponse") or {}
            if not isinstance(tool_response, dict):
                tool_response = {}
            status = str(tool_response.get("status") or "").lower()
            if status in ("async_launched", "running", "in_progress"):
                name = str(
                    tool_response.get("description")
                    or _get(
                        payload.get("tool_input") or payload.get("toolInput") or {},
                        "subagent_type",
                        "subagentType",
                        "description",
                        default=tool,
                    )
                )
                return {
                    "lifecycle": "active",
                    "current": {
                        "type": "subagent",
                        "name": name,
                        "summary": f"Background: {name}",
                        "startedAt": _now_iso(),
                    },
                    "attention": {"level": "none"},
                    "health": "ok",
                }
        return {
            "lifecycle": "active",
            "current": {
                "type": "tool",
                "name": tool,
                "summary": f"Finished {tool}",
                "startedAt": _now_iso(),
            },
            "attention": {"level": "none"},
            "health": "ok",
        }

    if event == "subagentstart":
        # Bracket: main session is running / waiting on a subagent → Running.
        name = str(_agent_type(payload) or _get(payload, "description", default="subagent"))
        return {
            "lifecycle": "active",
            "current": {
                "type": "subagent",
                "name": name,
                "summary": f"Subagent: {name}",
                "startedAt": _now_iso(),
            },
            "attention": {"level": "none"},
            "health": "ok",
        }

    if event == "subagentstop":
        # One child finished. If other background work remains → still Running;
        # otherwise main is continuing (thinking), not human-idle yet.
        name = str(
            _agent_type(payload)
            or _get(payload, "tool_name", "toolName", "description", default="subagent")
        )
        bg = _background_tasks(payload)
        if bg:
            return _background_work_facets(payload, bg)
        return {
            "lifecycle": "active",
            "current": {
                "type": "thinking",
                "name": name,
                "summary": f"Subagent finished: {name}",
                "startedAt": _now_iso(),
            },
            "attention": {"level": "none"},
            "health": "ok",
        }

    if event in ("posttoolusefailure", "stopfailure"):
        tool = str(_get(payload, "tool_name", "toolName", default="tool"))
        err = _get(payload, "error", default=None)
        summary = f"Failed: {tool}"
        if err and event == "stopfailure":
            summary = _truncate(str(err), 120) or summary
        return {
            "lifecycle": "active",
            "current": {
                "type": "tool",
                "name": tool,
                "summary": summary,
            },
            "attention": {
                "level": "informational",
                "reason": "failure",
                "title": f"{tool} failed" if event != "stopfailure" else "Turn failed",
            },
            "health": "degraded",
        }

    if event in ("permissionrequest", "permissiondenied"):
        tool = str(_get(payload, "tool_name", "toolName", default="tool"))
        return {
            "lifecycle": "active",
            "current": {
                "type": "waiting",
                "summary": f"Permission: {tool}",
            },
            "attention": {
                "level": "required",
                "reason": "approval",
                # Honest: approve in the agent UI — Nerve does not reverse-control.
                "title": "Approval needed in agent",
                "summary": _truncate(
                    f"{tool}: "
                    + json.dumps(
                        _get(payload, "tool_input", "toolInput", default={}),
                        default=str,
                        separators=(",", ":"),
                    ),
                    140,
                ),
            },
            "health": "ok",
        }

    if event == "notification":
        # Claude Code documents notification_type as a closed enum — use that only.
        # Never classify by message/title text.
        ntype = _notification_type(payload)
        title = str(_get(payload, "title", "message", default="") or "")
        summary = _truncate(title, 120) or ntype or "Notification"
        bg = _background_tasks(payload)

        if ntype in ("permission_prompt", "permission"):
            return {
                "lifecycle": "active",
                "current": {"type": "waiting", "summary": summary},
                "attention": {
                    "level": "required",
                    "reason": "approval",
                    "title": "Approval needed in agent",
                    "summary": summary or "Return to the agent to approve",
                },
                "health": "ok",
            }

        # idle_prompt = human’s turn only when no bg work remains *and* the toast
        # is not a harness shell / monitor / background-agent wait line.
        if ntype in ("idle_prompt", "agent_needs_input", "elicitation_dialog"):
            if bg:
                return _background_work_facets(payload, bg)
            # agent_needs_input: a (background) agent needs the human → Attention.
            # idle_prompt + bg-wait toast: shell/subagent → Running; monitor → Monitor.
            if ntype == "idle_prompt":
                return None  # Preserve the last real state.
            # Honest copy: Nerve signals "go back to agent UI", never "type here".
            return _your_turn(
                summary
                if summary and ntype != "idle_prompt"
                else "Return to the agent to continue"
            )

        if ntype in ("agent_completed", "elicitation_complete", "elicitation_response", "auth_success"):
            return {
                "lifecycle": "active",
                "current": {"type": "info", "summary": summary},
                "attention": {"level": "none"},
                "health": "ok",
            }

        # Unknown / missing type (e.g. Grok UI toasts): keep session active Running.
        # Do not invent Attention from free-text. Real idle is Stop / idle_prompt.
        if bg:
            return _background_work_facets(payload, bg)
        if _is_harness_background_wait_toast(summary):
            return _facets_for_bg_wait_toast(summary)
        return {
            "lifecycle": "active",
            "current": {"type": "info", "summary": summary},
            "attention": {"level": "none"},
            "health": "ok",
        }

    if event == "stop":
        if str(payload.get("reason", "")).strip() in ("channel_closed", "shutdown"):
            return None
        if str(payload.get("stopHookActive", payload.get("stop_hook_active", False))).lower() == "true":
            return {"lifecycle": "active", "current": {"type": "thinking", "summary": "Continuing"}, "attention": {"level": "none"}, "health": "ok"}
        bg = _background_tasks(payload)
        if bg:
            return _background_work_facets(payload, bg)
        return {
            "lifecycle": "active",
            "current": {"type": "completed", "summary": "Turn complete"},
            "attention": {"level": "none"},
            "health": "ok",
        }

    if event in ("stopcancelled", "elicitation"):
        return _your_turn("Return to the agent to continue")

    if event == "cwdchanged":
        return {
            "lifecycle": "active",
            "current": {"type": "info", "summary": "Workspace changed"},
            "attention": {"level": "none"},
            "health": "ok",
        }

    if event == "elicitationresult":
        text = _truncate(_get(payload, "summary", "title", default=""), 120)
        return {
            "lifecycle": "active",
            "current": {"type": "info", "summary": text or "Elicitation result"},
            "attention": {"level": "none"},
            "health": "ok",
        }

    if event in ("taskcreated", "taskcompleted"):
        label = "Task completed" if event == "taskcompleted" else "Task created"
        name = _get(payload, "title", "name")
        text = _get(payload, "title", "name", "description", default="")
        current: dict[str, Any] = {
            "type": "info",
            "summary": f"{label}: {text}" if text else label,
        }
        if name:
            current["name"] = str(name)
        return {
            "lifecycle": "active",
            "current": current,
            "attention": {"level": "none"},
            "health": "ok",
        }

    if event == "teammateidle":
        # A teammate going idle is not a human Ask.
        name = _get(payload, "name", "agent_type", "agentType", "description")
        current = {
            "type": "info",
            "summary": f"Teammate idle: {name}" if name else "Teammate idle",
        }
        if name:
            current["name"] = str(name)
        return {
            "lifecycle": "active",
            "current": current,
            "attention": {"level": "none"},
            "health": "ok",
        }

    if event == "precompact":
        # Compacting is work. Keep any reported background tasks; otherwise thinking.
        bg = _background_tasks(payload)
        if bg:
            return _background_work_facets(payload, bg)
        return {
            "lifecycle": "active",
            "current": {
                "type": "thinking",
                "summary": "Compacting context",
                "startedAt": _now_iso(),
            },
            "attention": {"level": "none"},
            "health": "ok",
        }

    if event == "postcompact":
        bg = _background_tasks(payload)
        if bg:
            return _background_work_facets(payload, bg)
        return {"lifecycle": "active", "current": {"type": "thinking", "summary": "Context compacted — continuing"}, "attention": {"level": "none"}, "health": "ok"}

    return None


def _file_uri(path: str) -> str | None:
    """Percent-encoded file:// URI for an absolute path (or None if not absolute).

    Absoluteness is asked of ``Path``, not of a leading slash: this hook runs on
    the machine that owns ``cwd``, so the host's own rules are the right ones,
    and ``C:\\work\\nerve`` used to answer None on the very platform where it is
    absolute. ``as_uri`` then emits ``file:///C:/work/nerve``, which is what the
    surfaces decode.

    Uses the path as given (no resolve) so unit tests and missing dirs still work.
    """
    if not path:
        return None
    try:
        candidate = Path(path)
    except (TypeError, ValueError):
        return None
    if not candidate.is_absolute():
        return None
    try:
        return candidate.as_uri()
    except ValueError:
        # Extremely odd paths — last-resort encoding.
        from urllib.parse import quote

        return "file://" + quote(str(path).replace("\\", "/"), safe="/:")


def _detect_ide_scheme() -> str | None:
    """Return cursor / vscode scheme when the host is clearly an editor, else None.

    Terminal CLIs (Claude Code / Codex / Grok in iTerm) leave this None so we
    fall back to a workspace file:// open rather than a dead IDE deep link.
    """
    term = (os.environ.get("TERM_PROGRAM") or "").lower()
    if (
        os.environ.get("CURSOR_TRACE_ID")
        or os.environ.get("CURSOR_AGENT")
        or "cursor" in term
        or "cursor" in (os.environ.get("TERM_PROGRAM_VERSION") or "").lower()
    ):
        return "cursor"
    if (
        os.environ.get("VSCODE_INJECTION")
        or os.environ.get("VSCODE_PID")
        or os.environ.get("VSCODE_GIT_IPC_HANDLE")
        or term in ("vscode", "vscode-insiders")
    ):
        return "vscode"
    return None


def _terminal_label() -> str | None:
    """Short human label for the hosting terminal/tab when known."""
    mapping = (
        ("ITERM_SESSION_ID", "iTerm"),
        ("TERM_SESSION_ID", "Terminal"),
        ("WEZTERM_PANE", "WezTerm"),
        ("KITTY_WINDOW_ID", "Kitty"),
        ("TMUX_PANE", "tmux"),
    )
    for key, label in mapping:
        if os.environ.get(key):
            return label
    term = os.environ.get("TERM_PROGRAM")
    if term:
        return term
    return None


def _build_location(
    payload: dict[str, Any],
    producer_key: str,
    cwd: str,
) -> dict[str, Any]:
    """Focus-first location: reliable openURL + human focusHint for the agent UI.

    openURL preference:
      1. IDE deep link when host is Cursor / VS Code (``cursor://file/…``)
      2. Workspace ``file://`` URI (opens Finder / default folder handler)
    focusHint is always a paste-friendly breadcrumb: producer · project · host.
    """
    meta = PRODUCER_META.get(
        producer_key,
        {"id": producer_key, "name": producer_key.title()},
    )
    project = _project_name(cwd)
    producer_name = meta.get("name") or producer_key
    host = _terminal_label() or "session"
    focus_hint = f"{producer_name} · {project} · {host}"
    if cwd:
        focus_hint = f"{focus_hint} · {cwd}"

    open_url: str | None = None
    scheme = _detect_ide_scheme()
    if scheme and cwd and Path(cwd).is_absolute():
        # vscode://file/Users/… and cursor://file/Users/… (no extra slash).
        # A drive path needs the URL's own slash: vscode://file/C:/work/nerve.
        target = str(cwd).replace("\\", "/")
        if not target.startswith("/"):
            target = "/" + target
        open_url = f"{scheme}://file{target}"
    else:
        open_url = _file_uri(cwd)

    loc: dict[str, Any] = {
        "openURL": open_url,
        "focusHint": focus_hint,
        "logPath": _get(payload, "transcript_path", "transcriptPath", default=None),
    }
    return {k: v for k, v in loc.items() if v is not None}


def _local_actions(location: dict[str, Any]) -> list[dict[str, Any]]:
    """Display-only actions: Open/Focus first, then Copy, then Open logs.

    Never approve/submit — Nerve does not reverse-control (invariant 6).
    """
    actions: list[dict[str, Any]] = []
    has_url = bool(location.get("openURL"))
    has_hint = bool(location.get("focusHint"))
    if has_url or has_hint:
        actions.append(
            {
                "id": "open",
                "title": "Open" if has_url else "Focus",
                "kind": "open" if has_url else "focus",
                "state": "available",
                "destructive": False,
                "confirmationRequired": False,
            }
        )
    actions.append(
        {
            "id": "copy",
            "title": "Copy",
            "kind": "copy_summary",
            "state": "available",
            "destructive": False,
            "confirmationRequired": False,
        }
    )
    # macOS ActionService already reveals `location.logPath` on this action id.
    if location.get("logPath"):
        actions.append(
            {
                "id": "open_logs",
                "title": "Open logs",
                "kind": "open_logs",
                "state": "available",
                "destructive": False,
                "confirmationRequired": False,
            }
        )
    return actions


def _build_job(
    payload: dict[str, Any],
    producer_key: str,
    session_id: str,
    facets: dict[str, Any],
) -> dict[str, Any]:
    meta = PRODUCER_META.get(
        producer_key,
        {"id": producer_key, "name": producer_key.title(), "kind": f"agent.{producer_key}"},
    )
    cwd = _cwd(payload)
    project = _project_name(cwd)
    # Job title = project (cwd basename). Producer is its own field — do not
    # bake "Claude Code — nerve" into the name; the panel already has status +
    # current.summary for what is happening.
    now = _now_iso()
    alias = _machine_alias()
    job_id = f"{meta['id']}:{session_id}"

    extensions: dict[str, Any] = {
        "hookEvent": _get(payload, "hook_event_name", "hookEventName", "event", default=""),
        "sessionId": session_id,
        "model": _get(payload, "model", default=None),
        # UI slot + producer PID so Nerve can supersede ghosts and reap dead locals.
        "slot": _slot_id(producer_key, payload),
    }
    agent_pid = _agent_process_pid()
    if agent_pid:
        extensions["pid"] = agent_pid
    # Optional breadcrumbs when the main-session update came from a subagent bracket.
    at = _agent_type(payload)
    aid = _agent_id(payload)
    if at:
        extensions["agentType"] = at
    if aid and _event_name(payload) in ("subagentstart", "subagentstop"):
        extensions["agentId"] = aid
    if facets.get("end_reason"):
        extensions["endReason"] = facets["end_reason"]
    # The prompt only exists in this one hook run — `current` is overwritten by
    # the next event, so it travels as an extension the hub keeps (see
    # `crates/nerve-hub/src/state/store.rs` STICKY_EXTENSIONS).
    if _event_name(payload) == "userpromptsubmit":
        prompt = _truncate(_get(payload, "prompt", "user_prompt", default=""), PROMPT_MAX)
        if prompt:
            extensions["lastPrompt"] = prompt
            extensions["lastPromptAt"] = now

    location = _build_location(payload, producer_key, cwd)

    job: dict[str, Any] = {
        "id": job_id,
        "kind": "session",
        "name": project,
        "alias": alias,
        "lifecycle": facets["lifecycle"],
        "current": facets.get("current"),
        "attention": facets.get("attention") or {"level": "none"},
        "health": facets.get("health") or "ok",
        "progress": {"kind": "none"},
        "producer": {
            "id": meta["id"],
            "name": meta["name"],
            "kind": meta["kind"],
        },
        "context": {
            "project": project,
            "workspace": cwd,
            "labels": [producer_key, "session"],
        },
        "location": location,
        "capabilities": [],
        "actions": _local_actions(location),
        "createdAt": now,
        "startedAt": now,
        "updatedAt": now,
        "version": _version_ms(),
        "extensions": {
            k: v for k, v in extensions.items() if v is not None and v != ""
        },
    }

    if facets.get("outcome"):
        job["outcome"] = facets["outcome"]
    if facets.get("ended"):
        job["endedAt"] = now
        job["lifecycle"] = "ended"
    # current.startedAt is present on every non-ended job (decision 7) — the hub
    # already does this, and a missing stamp is three silent schemas.
    if not facets.get("ended") and job.get("current") is not None:
        job["current"].setdefault("startedAt", now)

    if not job.get("location"):
        job.pop("location", None)

    return job


def _post_json(path: str, payload: dict[str, Any]) -> bool:
    """Write `payload` to the fixed ingest endpoint. Never read the response.

    Framing is ``http.client``'s job, not this file's. The call returns once
    the request bytes have been handed to the socket, or the connect fails.
    The hub's status line is not the agent's business: a closed port, a 4xx
    and a hub that accepts and then sits there all return without raising, and
    nothing is written to stdout or stderr (a harness would show that to the
    model).

    The import is local and deliberate: it costs about 9 ms, and the hook runs
    as one process per agent event, so only events that actually post should
    pay for it.

    The endpoint is read from the module globals at call time rather than
    captured in a default argument, which is the seam the transport tests
    already use to point this at a stand-in on an ephemeral port. Production
    never moves it: the ingest address is fixed (CLAUDE.md invariant 2).
    """
    import http.client

    body = json.dumps(payload).encode("utf-8")
    connection = http.client.HTTPConnection(
        INGEST_HOST, INGEST_PORT, timeout=INGEST_TIMEOUT
    )
    try:
        connection.request(
            "POST",
            path,
            body=body,
            headers={
                "User-Agent": "nerve-hook/0.4",
                "Content-Type": "application/json",
                # One request per connection: no keep-alive to manage.
                "Connection": "close",
            },
        )
        return True
    except Exception:
        return False
    finally:
        try:
            connection.close()
        except Exception:
            pass


def _post_snapshot(alias: str, machine_kind: str, job: dict[str, Any]) -> bool:
    """POST one conversation row. Thin wrapper over [_post_jobs]."""
    return _post_jobs(alias, machine_kind, [job])


def _post_jobs(alias: str, machine_kind: str, jobs: list[dict[str, Any]]) -> bool:
    """POST one or more conversation rows in a single envelope.

    A SessionStart that supersedes a ghost closes the old conversation and opens
    the new one in one round trip — two conversations, not two jobs per
    conversation (invariant 1). Fail-open: a hub that is down is a silent miss.
    """
    if not jobs:
        return True
    payload = {
        "alias": alias,
        "machineKind": machine_kind,
        "jobs": jobs,
    }
    return _post_json("/v1/snapshot", payload)


# ---------------------------------------------------------------------------
# Session-slot memory — close previous conversation when hosts skip SessionEnd
# ---------------------------------------------------------------------------

_SHELL_NAMES = frozenset(
    {
        "bash",
        "sh",
        "zsh",
        "fish",
        "dash",
        "python",
        "python3",
        "python3.11",
        "python3.12",
        "python3.13",
        "run.sh",
    }
)


def _state_dir() -> Path:
    if _STATE_DIR_OVERRIDE is not None:
        return _STATE_DIR_OVERRIDE
    # Imported here rather than at module scope: `tempfile` pulls in `shutil`
    # and `random`, and an event the hook skips never reaches this.
    import tempfile

    d = Path(tempfile.gettempdir()) / "nerve-hook"
    try:
        d.mkdir(mode=0o700, exist_ok=True)
    except Exception:
        pass
    return d


# ---------------------------------------------------------------------------
# Identity cache — alias + agent pid survive the one-process-per-event shape
# ---------------------------------------------------------------------------

#: How long a cached alias/pid stays good. SessionStart invalidates it early.
_IDENTITY_TTL = 24 * 60 * 60


# Same list as nerve.js SLOT_ENV_KEYS. Windows Terminal and ConEmu are
# included so two concurrent Windows sessions do not collapse onto one pid.
_SLOT_ENV_KEYS = (
    "TERM_SESSION_ID",
    "ITERM_SESSION_ID",
    "WEZTERM_PANE",
    "KITTY_WINDOW_ID",
    "TMUX_PANE",
    "WT_SESSION",
    "ConEmuPID",
)


def _terminal_token() -> str:
    for key in _SLOT_ENV_KEYS:
        val = os.environ.get(key)
        if val:
            return f"{key}:{val}"
    return "default"


def _identity_path() -> Path:
    token = re.sub(r"[^\w.-]", "_", _terminal_token())
    return _state_dir() / f"identity-{token}.json"


def _identity_load() -> dict[str, Any] | None:
    try:
        data = json.loads(_identity_path().read_text(encoding="utf-8"))
        if not isinstance(data, dict):
            return None
        ts = data.get("ts")
        if not isinstance(ts, (int, float)) or (time.time() - float(ts)) > _IDENTITY_TTL:
            return None
        return data
    except Exception:
        return None


def _identity_save(alias: str, pid: int | None) -> None:
    try:
        _identity_path().write_text(
            json.dumps({"alias": alias, "pid": pid, "ts": time.time()}),
            encoding="utf-8",
        )
    except Exception:
        pass


def _identity_invalidate() -> None:
    try:
        _identity_path().unlink(missing_ok=True)
    except Exception:
        pass


@functools.lru_cache(maxsize=32)
def _proc_ppid_and_name(pid: int) -> tuple[int | None, str | None]:
    """Best-effort parent pid + command name (macOS / Linux).

    Cached per pid: the climb below asks about the same handful of processes
    from several callers, and each answer costs a `ps` spawn.
    """
    try:
        import subprocess

        out = subprocess.check_output(
            ["ps", "-p", str(pid), "-o", "ppid=,comm="],
            text=True,
            stderr=subprocess.DEVNULL,
            timeout=0.4,
        ).strip()
        if not out:
            return None, None
        parts = out.split(None, 1)
        ppid = int(parts[0]) if parts else None
        name = parts[1].strip() if len(parts) > 1 else None
        if name:
            name = Path(name).name
        return ppid, name
    except Exception:
        return None, None


@functools.lru_cache(maxsize=1)
def _agent_process_pid() -> int | None:
    """Climb past shell/python wrappers to the agent host PID for this slot.

    The single most expensive thing the hook does — up to six `ps` spawns — and
    four callers want it (the job body, the slot id, twice more when a
    SessionStart supersedes the previous conversation). Cached in-process *and*
    on disk keyed by the terminal token, so only the first event per UI slot
    pays the climb.
    """
    cached = _identity_load()
    if cached and "pid" in cached:
        pid_val = cached.get("pid")
        return pid_val if isinstance(pid_val, int) and pid_val > 1 else None
    result = _climb_agent_pid()
    _identity_save(_machine_alias(), result)
    return result


def _climb_agent_pid() -> int | None:
    try:
        pid = os.getppid()
    except Exception:
        return None
    last = pid
    for _ in range(6):
        if pid is None or pid <= 1:
            return last if last and last > 1 else None
        ppid, name = _proc_ppid_and_name(pid)
        base = (name or "").lower()
        if base and base not in _SHELL_NAMES and not base.endswith(".sh"):
            return pid
        last = pid
        if ppid is None or ppid == pid:
            return last
        pid = ppid
    return last


def _host_slot_token() -> str:
    """Identify the UI/terminal/process that owns this conversation.

    Same env keys as ``_terminal_token`` / nerve.js ``SLOT_ENV_KEYS``. Prefer
    that token (stable across /new in the same tab). Fall back to the agent
    host PID so two concurrent windows do not thrash each other.
    """
    token = _terminal_token()
    if token != "default":
        return token
    agent_pid = _agent_process_pid()
    if agent_pid:
        return f"pid:{agent_pid}"
    return "default"


def _slot_id(producer_key: str, payload: dict[str, Any]) -> str:
    alias = _machine_alias()
    # cwd is NOT part of the key: /new often keeps the same project dir.
    raw = f"{producer_key}\0{alias}\0{_host_slot_token()}"
    return hashlib.sha256(raw.encode("utf-8")).hexdigest()[:32]


def _slot_path(slot_id: str) -> Path:
    return _state_dir() / f"{slot_id}.json"


def _slot_read(slot_id: str) -> str | None:
    path = _slot_path(slot_id)
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
        sid = data.get("session_id")
        return str(sid) if sid else None
    except Exception:
        return None


def _slot_write(slot_id: str, session_id: str) -> None:
    path = _slot_path(slot_id)
    try:
        path.parent.mkdir(mode=0o700, exist_ok=True)
        tmp = path.with_suffix(".tmp")
        tmp.write_text(
            json.dumps({"session_id": session_id, "ts": _now_iso()}),
            encoding="utf-8",
        )
        tmp.replace(path)
    except Exception:
        pass


def _slot_clear(slot_id: str, session_id: str | None = None) -> None:
    path = _slot_path(slot_id)
    try:
        if session_id is not None:
            current = _slot_read(slot_id)
            if current is not None and current != session_id:
                return
        path.unlink(missing_ok=True)
    except Exception:
        pass


def _ended_facets(
    *,
    summary: str = "Session ended",
    reason: str | None = None,
    outcome: str = "cancelled",
) -> dict[str, Any]:
    facets: dict[str, Any] = {
        "lifecycle": "ended",
        "outcome": outcome,
        "current": {"type": "idle", "summary": summary},
        "attention": {"level": "none"},
        "health": "ok",
        "ended": True,
    }
    if reason:
        facets["end_reason"] = reason
    return facets


def _ended_job(
    payload: dict[str, Any],
    producer_key: str,
    session_id: str,
    *,
    summary: str,
    reason: str,
) -> dict[str, Any] | None:
    facets = _ended_facets(summary=summary, reason=reason)
    job = _build_job(payload, producer_key, session_id, facets)
    if str(job.get("id", "")).count(":") >= 2:
        return None
    ext = job.setdefault("extensions", {})
    ext["endReason"] = reason
    return job


def _supersede_ghost(
    payload: dict[str, Any],
    producer_key: str,
    session_id: str,
) -> dict[str, Any] | None:
    """Build (but do not POST) the ended row for a session this slot replaced.

    Covers /new, /clear, fork, and hosts that omit SessionEnd. The caller
    batches it with the new row into a single snapshot.
    """
    slot = _slot_id(producer_key, payload)
    previous = _slot_read(slot)
    if previous and previous != session_id:
        return _ended_job(
            payload,
            producer_key,
            previous,
            summary="Session ended (superseded)",
            reason="superseded",
        )
    return None


def process(payload: dict[str, Any]) -> dict[str, Any] | None:
    """Map one hook event → a single main-session job snapshot (or None to skip).

    Always one conversation job. Never emits parentJobId / paintRibbon / child ids.
    On SessionStart, may also POST an ended snapshot for the previous session in
    the same UI slot when the host skipped SessionEnd (e.g. /new).
    """
    event = _event_name(payload)
    if not event:
        return None

    # Silent-noop events exit before alias / slot / ps / HTTP / disk (Phase 4).
    if event in _SILENT_NOOP:
        return None

    if event not in {
        "sessionstart",
        "setup",
        "sessionend",
        "userpromptsubmit",
        "userpromptexpansion",
        "pretooluse",
        "posttooluse",
        "posttoolusefailure",
        "posttoolbatch",
        "permissionrequest",
        "permissiondenied",
        "notification",
        "messagedisplay",
        "stop",
        "stopfailure",
        "stopcancelled",
        "subagentstart",
        "subagentstop",
        "taskcreated",
        "taskcompleted",
        "teammateidle",
        "instructionsloaded",
        "configchange",
        "cwdchanged",
        "directoryadded",
        "filechanged",
        "precompact",
        "postcompact",
        "elicitation",
        "elicitationresult",
    }:
        return None

    # Inside a subagent: ignore tool chatter so the main session row is not
    # rewritten on every Bash/Read. SubagentStart/Stop and Permission still apply
    # to the main session (those events either bracket work or need Attention).
    if _agent_id(payload) and event in _SUBAGENT_INTERNAL_NOISE:
        return None

    facets = _map_event(event, payload)
    if facets is None:
        return None

    producer_key = _detect_producer(payload)
    session_id = _session_id(payload)
    # A new conversation is the moment a stale identity is most wrong (renamed
    # machine, restarted host) — drop the cache so the climb runs once here.
    if event == "sessionstart":
        _identity_invalidate()
    slot = _slot_id(producer_key, payload)

    pending: list[dict[str, Any]] = []
    # /new · /clear · fork: new SessionStart often arrives without SessionEnd.
    if event == "sessionstart":
        ghost = _supersede_ghost(payload, producer_key, session_id)
        if ghost is not None:
            pending.append(ghost)

    # Guard: session_id must never be the subagent id alone when both exist.
    # Job id is always {producer}:{session_id} with exactly one colon separator
    # between producer id and session (producer id may itself contain hyphens).
    job = _build_job(payload, producer_key, session_id, facets)
    # Hard invariant — refuse to post accidental child-shaped ids.
    if str(job.get("id", "")).count(":") >= 2:
        return None
    if "parentJobId" in (job.get("extensions") or {}):
        return None

    pending.append(job)
    # One POST carries the ghost and the new row: two conversations, not two
    # jobs per conversation (invariant 1).
    _post_jobs(job["alias"], _machine_kind(), pending)

    if event == "sessionend":
        _slot_clear(slot, session_id)
    else:
        # Remember the live conversation for this UI slot.
        _slot_write(slot, session_id)

    return job


def _map_fixture(path: str) -> int:
    """Test-only adapter: ``nerve.py --map <fixture.json>`` prints raw facets or ``null``.

    Not an env flag and not ``NERVE_*`` (invariant 2); production hooks never
    pass argv. Exits 0 even on a bad path so a harness can never wedge.
    """
    out = "null"
    try:
        payload = json.loads(Path(path).read_text(encoding="utf-8"))
        if isinstance(payload, dict):
            facets = _map_event(_event_name(payload), payload)
            out = json.dumps(facets, ensure_ascii=False) if facets is not None else "null"
    except Exception:
        out = "null"
    try:
        # Windows text stdout uses the ANSI code page even on a pipe, so an em
        # dash becomes a single byte the parity harness cannot read as UTF-8.
        # The adapter's contract is UTF-8 JSON, same as nerve.js and the hub.
        sys.stdout.buffer.write((out + "\n").encode("utf-8"))
        sys.stdout.buffer.flush()
    except Exception:
        pass
    return 0


def main() -> int:
    if len(sys.argv) >= 3 and sys.argv[1] == "--map":
        return _map_fixture(sys.argv[2])
    try:
        raw = sys.stdin.read()
    except Exception:
        return 0
    if not raw or not raw.strip():
        return 0
    try:
        payload = json.loads(raw)
    except json.JSONDecodeError:
        return 0
    if not isinstance(payload, dict):
        return 0
    try:
        process(payload)
    except Exception:
        pass
    return 0


if __name__ == "__main__":
    sys.exit(main())
