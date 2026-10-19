"""Codex adapter tests; lifecycle semantics are tested in nerve-hub."""
import sys
sys.dont_write_bytecode = True
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch, MagicMock

path = Path(__file__).resolve().parents[3] / "plugins/nerve/hooks/nerve.py"
spec = importlib.util.spec_from_file_location("nerve", path)
hook = importlib.util.module_from_spec(spec)
spec.loader.exec_module(hook)

class AdapterTests(unittest.TestCase):
    def test_terminal_identity_is_raw_and_optional(self):
        with patch.object(hook, 'agent_pid', return_value=123), patch.object(hook.subprocess, 'check_output', return_value='ttys003\n'), patch.object(hook.socket, 'gethostname', return_value='Studio'):
            event = hook.enrich({'session_id':'a'})
            self.assertEqual(event['terminalHost'], 'studio')
            self.assertEqual(event['terminalName'], '/dev/ttys003')
        with patch.object(hook, 'agent_pid', return_value=123), patch.object(hook.subprocess, 'check_output', side_effect=OSError()):
            self.assertNotIn('terminalName', hook.enrich({}))
    def test_raw_event_and_metadata_only(self):
        payload = {"hook_event_name":"Stop", "session_id":"s", "background_tasks":[{"type":"shell"}]}
        socket = MagicMock()
        with patch.object(hook.socket, "create_connection", return_value=socket), patch.object(hook, "agent_pid", return_value=123):
            hook.forward(payload)
        sent = socket.__enter__.return_value.sendall.call_args.args[0]
        self.assertIn(b"POST /v1/hook?producer=codex", sent)
        self.assertIn(b'"session_id": "s"', sent)
        self.assertIn(b'"agentPid": 123', sent)
        self.assertNotIn(b'"jobs"', sent)
        self.assertNotIn("agentPid",payload)
    def test_invalid_json_is_fail_open(self):
        with patch.object(hook.json,"load",side_effect=ValueError("invalid")):
            self.assertEqual(hook.main(),0)
    def test_same_slot_for_new_session(self):
        with patch.object(hook,"agent_pid",return_value=123):
            self.assertEqual(hook.enrich({"session_id":"a"})["slotId"],hook.enrich({"session_id":"b"})["slotId"])
if __name__ == "__main__": unittest.main()
