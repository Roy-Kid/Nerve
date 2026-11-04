"""Every official host event has a registered slot in the marketplace plugin."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
HOOKS = ROOT / "plugins" / "nerve" / "hooks"

# Official event names. Claude omits WorktreeCreate/WorktreeRemove: a no-op
# hook fails worktree creation (the contract requires a path on stdout).
CLAUDE = {
    "SessionStart",
    "Setup",
    "UserPromptSubmit",
    "UserPromptExpansion",
    "PreToolUse",
    "PermissionRequest",
    "PermissionDenied",
    "PostToolUse",
    "PostToolUseFailure",
    "PostToolBatch",
    "Notification",
    "MessageDisplay",
    "SubagentStart",
    "SubagentStop",
    "TaskCreated",
    "TaskCompleted",
    "Stop",
    "StopCancelled",
    "StopFailure",
    "TeammateIdle",
    "InstructionsLoaded",
    "ConfigChange",
    "CwdChanged",
    "DirectoryAdded",
    "FileChanged",
    "PreCompact",
    "PostCompact",
    "Elicitation",
    "ElicitationResult",
    "SessionEnd",
}

CODEX = {
    "SessionStart",
    "SessionEnd",
    "UserPromptSubmit",
    "PreToolUse",
    "PermissionRequest",
    "PermissionDenied",
    "PostToolUse",
    "PostToolUseFailure",
    "PreCompact",
    "PostCompact",
    "Stop",
    "StopCancelled",
    "StopFailure",
    "SubagentStart",
    "SubagentStop",
    "TaskCreated",
    "TaskCompleted",
    "TeammateIdle",
    "CwdChanged",
    "Elicitation",
    "ElicitationResult",
}

GROK = {
    "SessionStart",
    "SessionEnd",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "PermissionDenied",
    "Stop",
    "StopCancelled",
    "StopFailure",
    "Notification",
    "SubagentStart",
    "SubagentStop",
    "PreCompact",
    "PostCompact",
    "TaskCreated",
    "TaskCompleted",
    "TeammateIdle",
    "CwdChanged",
    "Elicitation",
    "ElicitationResult",
}


def events(name: str) -> set[str]:
    data = json.loads((HOOKS / name).read_text())
    return set(data["hooks"])


class HookSlotTests(unittest.TestCase):
    def test_claude_registers_every_safe_official_event(self):
        self.assertEqual(events("hooks.json"), CLAUDE)

    def test_codex_registers_every_official_event(self):
        self.assertEqual(events("codex.json"), CODEX)

    def test_grok_registers_every_official_event(self):
        self.assertEqual(events("grok.json"), GROK)




class CodexWindowsOverrideTests(unittest.TestCase):
    """Every Codex slot carries its Windows command as well as its unix one.

    Codex takes the command as a shell string, and `python3` on Windows is a
    Microsoft Store alias stub that opens the Store rather than running
    anything. `commandWindows` is Codex's own documented override for exactly
    this (learn.chatgpt.com/docs/hooks); without it a Windows Codex user gets
    no status at all, silently, because hooks fail open.
    """

    def setUp(self):
        self.config = json.loads((HOOKS / "codex.json").read_text())

    def _entries(self):
        for event, groups in self.config["hooks"].items():
            for group in groups:
                for entry in group["hooks"]:
                    yield event, entry

    def test_every_command_hook_has_a_windows_override(self):
        for event, entry in self._entries():
            if entry.get("type") != "command":
                continue
            self.assertIn(
                "commandWindows",
                entry,
                f"{event} would run `python3` on Windows, which is a Store stub",
            )

    def test_the_windows_command_uses_the_py_launcher(self):
        # `py -3` is what every python.org install ships; `python3` is not.
        for event, entry in self._entries():
            command = entry.get("commandWindows")
            if command is None:
                continue
            self.assertTrue(
                command.startswith("py -3 "),
                f"{event} uses {command!r} rather than the py launcher",
            )


if __name__ == "__main__":
    unittest.main()
