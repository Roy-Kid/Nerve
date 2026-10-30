#!/usr/bin/env python3
"""Pure Codex event forwarder. Task state and session memory belong to the hub."""
import hashlib
import json
import os
import re
from pathlib import Path
import socket
import subprocess
import sys
sys.dont_write_bytecode = True

SLOT_KEYS = ("TERM_SESSION_ID", "ITERM_SESSION_ID", "WEZTERM_PANE", "KITTY_WINDOW_ID", "TMUX_PANE", "WT_SESSION", "ConEmuPID")

def agent_pid():
    pid = os.getppid()
    if sys.platform == "win32":
        return pid
    for _ in range(6):
        try:
            line = subprocess.check_output(["ps", "-p", str(pid), "-o", "ppid=,comm="], text=True, stderr=subprocess.DEVNULL, timeout=0.1).strip()
            parent, name = line.split(None, 1)
            name = Path(name).name.lower()
            # node and python are often the agent. Only a shell wrapper is skipped.
            if name not in {"sh", "bash", "zsh", "fish", "dash"} and not name.endswith(".sh"):
                break
            parent = int(parent)
            if parent <= 1 or parent == pid:
                break
            pid = parent
        except Exception:
            break
    return pid if pid > 1 else None

def terminal_identity():
    """The session tty. A hook is often spawned off-tty; walk ancestors."""
    if sys.platform == 'win32':
        return None
    pid = os.getpid()
    seen = set()
    for _ in range(8):
        if pid <= 1 or pid in seen:
            break
        seen.add(pid)
        try:
            line = subprocess.check_output(['ps', '-p', str(pid), '-o', 'ppid=,tty='], text=True,
                stderr=subprocess.DEVNULL, timeout=0.1).strip()
        except (OSError, subprocess.SubprocessError):
            break
        parts = line.split(None, 1)
        if len(parts) != 2:
            break
        parent, tty = parts
        name = tty.removeprefix('/dev/')
        if re.fullmatch(r'ttys\d+|pts/\d+', name):
            return socket.gethostname().lower(), '/dev/' + name
        try:
            parent = int(parent)
        except ValueError:
            break
        if parent <= 1 or parent == pid:
            break
        pid = parent
    return None

def enrich(payload):
    result = dict(payload)
    pid = agent_pid()
    token = next((f"{key}:{os.environ[key]}" for key in SLOT_KEYS if os.environ.get(key)), f"pid:{pid}")
    result.update(agentPid=pid, slotId=hashlib.sha256(token.encode()).hexdigest()[:32])
    found = terminal_identity()
    if found:
        host, terminal = found
        result.update(terminalHost=host, terminalName=terminal)
    return result

def forward(payload):
    body = json.dumps(enrich(payload), ensure_ascii=False).encode("utf-8")
    header = ("POST /v1/hook?producer=codex HTTP/1.1\r\nHost: 127.0.0.1:17890\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: " + str(len(body)) + "\r\n\r\n").encode("ascii")
    with socket.create_connection(("127.0.0.1", 17890), timeout=0.25) as connection:
        connection.sendall(header + body)
        # Keep the request alive until the hub accepts it; still bounded and fail-open.
        try: connection.recv(1)
        except (TimeoutError, OSError): pass

def main():
    try:
        payload = json.load(sys.stdin)
        if isinstance(payload, dict):
            forward(payload)
    except Exception:
        pass
    return 0

if __name__ == "__main__":
    sys.exit(main())
