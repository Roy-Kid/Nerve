"""Nerve's checks: one function per CI job, and the commit runs its host's.

A CI job (`.github/workflows/ci.yml`) is setup, then `scripts/check.py --job
<name>`, and nothing else; `scripts/test_check.py` holds it to that. The
pre-commit hook runs the job this host can (`macos` on a Mac); the Windows
job runs on CI, on every push to dev or master.
"""
import argparse
import glob
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
PORTABLE = ["-p", "nerve-platform", "-p", "nerve-hub", "-p", "nerve-surface-core", "-p", "nerve-windows-surface"]


def run(*command: str, cwd: Path = ROOT) -> None:
    print("+ " + " ".join(command), flush=True)
    subprocess.run(command, cwd=cwd, check=True)


def macos() -> None:
    run(sys.executable, "scripts/test_check.py")
    run("cargo", "fmt", "--check")
    run("cargo", "clippy", "--workspace", "--all-targets", "--all-features", "--", "-D", "warnings")
    run("cargo", "test", "--workspace")
    # A green `cargo test` here says nothing about a `cfg(windows)` arm this
    # target never compiles; checking it costs no Windows machine.
    run("cargo", "check", *PORTABLE, "--all-targets", "--all-features", "--target", "x86_64-pc-windows-msvc")
    run(sys.executable, "sources/agents/tests/test_nerve_hook.py")
    run(sys.executable, "sources/agents/tests/test_hook_slots.py")
    run("node", "--test", "plugins/nerve/hooks/nerve.test.js")
    run(sys.executable, "scripts/check-plugin.py")
    run("npm", "ci", cwd=ROOT / "vsc-ext")
    run("npm", "test", cwd=ROOT / "vsc-ext")
    run("node", "--test", *sorted(glob.glob(str(ROOT / "surfaces/tether-web/tests/*.test.mjs"))))
    run("node", "--test", "testing/tether/model.test.cjs")
    run("npm", "ci", cwd=ROOT / "index")
    run("npm", "test", cwd=ROOT / "index")
    run("npm", "run", "build", cwd=ROOT / "index")


def windows() -> None:
    # nerve-tmux-surface is unix-only by design: tmux has no Windows port.
    run("cargo", "clippy", *PORTABLE, "--all-targets", "--all-features", "--", "-D", "warnings")
    run("cargo", "test", *PORTABLE, "--all-features")
    run("cargo", "build", "--release", "-p", "nerve-hub", "-p", "nerve-windows-surface")
    run("powershell", "-NoProfile", "-File", "scripts/nerve.ps1", "-VerifyLoop", "-BinaryDirectory", "./target/release")
    run("powershell", "-NoProfile", "-File", "scripts/test-windows-install.ps1")
    # The download CI uploads: both binaries, the launcher and the demo fixture.
    package = ROOT / "package"
    for directory in (package / "scripts", package / "fixtures"):
        directory.mkdir(parents=True, exist_ok=True)
    for binary in ("nerve-hub.exe", "nerve-windows-surface.exe"):
        shutil.copy2(ROOT / "target/release" / binary, package)
    shutil.copy2(ROOT / "scripts/nerve.ps1", package / "scripts")
    shutil.copy2(ROOT / "fixtures/demo_snapshot.json", package / "fixtures")


JOBS = {"macos": macos, "windows": windows}
HOST_JOBS = {"darwin": ("macos",), "win32": ("windows",)}


def one_at_a_time():
    """Two checks in one checkout share its build directories and the window
    server; the second would only wait on the first."""
    path = ROOT / "target" / "check.lock"
    path.parent.mkdir(exist_ok=True)
    lock = open(path, "a+")
    try:
        if sys.platform == "win32":
            import msvcrt
            msvcrt.locking(lock.fileno(), msvcrt.LK_NBLCK, 1)
        else:
            import fcntl
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        return None
    return lock


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--job", choices=sorted(JOBS), action="append",
                        help="the CI job to run; repeatable. Default: every job this host can run")
    args = parser.parse_args()
    held = one_at_a_time()  # released when the process ends
    if held is None:
        print("Another check is already running in this checkout.", file=sys.stderr)
        return 1
    jobs = args.job or HOST_JOBS.get(sys.platform, ())
    if not jobs:
        print(f"No CI job runs on {sys.platform}; nothing to check.", file=sys.stderr)
        return 1
    for job in jobs:
        print(f"== job {job}", flush=True)
        JOBS[job]()
    others = sorted(set(JOBS) - set(jobs))
    print("PASS: " + ", ".join(jobs) + (". Left to CI: " + ", ".join(others) if others else "") + ".")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except subprocess.CalledProcessError as error:
        print(f"BLOCKED: {error}", file=sys.stderr)
        sys.exit(1)
