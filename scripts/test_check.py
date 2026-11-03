"""The workflow runs what the commit runs: setup, then one gate job."""
import importlib.util
from pathlib import Path
import re
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


check = load("check")
ship = load("ship")
SETUP = re.compile(r"^$")  # Nothing but the gate runs in a job.


def workflow_runs(text: str) -> dict[str, list[str]]:
    """Each job's `run:` commands, read line by line: no YAML library is
    needed for a file this plain, and none is installed on every host."""
    jobs: dict[str, list[str]] = {}
    lines = text.splitlines()
    in_jobs, job, index = False, None, 0
    while index < len(lines):
        line = lines[index]
        if re.match(r"^jobs:\s*$", line):
            in_jobs = True
        elif in_jobs and (found := re.match(r"^  ([\w-]+):\s*$", line)):
            job = found.group(1)
            jobs[job] = []
        elif job and (found := re.match(r"^(\s+)(?:- )?run:\s*(.*)$", line)):
            value = found.group(2).strip()
            if value in ("|", ">"):
                body = []
                while index + 1 < len(lines) and (not lines[index + 1].strip()
                                                  or len(lines[index + 1]) - len(lines[index + 1].lstrip()) > len(found.group(1))):
                    index += 1
                    body.append(lines[index].strip())
                value = "\n".join(part for part in body if part)
            jobs[job].append(value)
        index += 1
    return jobs


class ParityTests(unittest.TestCase):
    def test_every_ci_job_is_setup_and_one_gate_job(self):
        seen = set()
        for path in sorted((ROOT / ".github/workflows").glob("*.yml")):
            for name, commands in workflow_runs(path.read_text(encoding="utf-8")).items():
                checks = []
                for command in commands:
                    found = re.fullmatch(r"python3? scripts/check\.py --job (\w+)", command)
                    if found:
                        checks.append(found.group(1))
                    else:
                        self.assertRegex(command, SETUP, f"{path.name} {name} runs outside the gate")
                self.assertEqual(checks, [name], f"{path.name} {name}")
                seen.add(name)
        self.assertEqual(seen, set(check.JOBS), "every gate job runs in CI, and only those")

    def test_a_job_name_and_its_run_lines_are_read(self):
        text = "on:\n  push:\njobs:\n  macos:\n    steps:\n      - run: python3 scripts/check.py --job macos\n" \
               "      - name: x\n        run: |\n          echo a\n          echo b\n  windows:\n    steps: []\n"
        self.assertEqual(workflow_runs(text), {"macos": ["python3 scripts/check.py --job macos", "echo a\necho b"],
                                               "windows": []})


class ShipTests(unittest.TestCase):
    def push(self, branch, sha="a" * 40, green=False):
        with patch.object(ship, "verdict", return_value=(green, [] if green else ["CI"])) as verdict:
            return ship.guard([(sha, branch)]), verdict

    def test_an_untried_commit_cannot_reach_master(self):
        self.assertEqual(self.push("master")[0], 1)

    def test_a_commit_every_workflow_passed_goes_through(self):
        self.assertEqual(self.push("master", green=True)[0], 0)

    def test_staging_branches_and_deletions_are_not_asked(self):
        code, verdict = self.push("ci/abc")
        self.assertEqual(code, 0)
        self.assertEqual(self.push("master", sha="0" * 40)[0], 0)
        verdict.assert_not_called()


if __name__ == "__main__":
    unittest.main()
