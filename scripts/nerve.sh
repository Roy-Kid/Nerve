#!/usr/bin/env bash
# Nerve — single dev + verify launcher (explicit flags only; no surprise demo data).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# The hub port is fixed — it doubles as the single-instance lock
# (crates/nerve-hub/src/cli.rs), so there is nothing here to parameterise.
readonly NERVE_BASE="http://127.0.0.1:17890"
readonly NERVE_FIXTURE="$ROOT/fixtures/demo_snapshot.json"
readonly NERVE_SURFACE_FIXTURE="$ROOT/fixtures/surface_demo_snapshot.json"
readonly NERVE_APP="$ROOT/Nerve/build/Build/Products/Debug/Nerve.app"

# What the EXIT traps below clean up. Script scope on purpose: a trap fires
# after the function that armed it has returned, and `set -u` turns a local
# that went out of scope into a failure on the way out ("nerve_hub_pid: unbound
# variable") — after the checks themselves have already passed.
nerve_hub_pid=""
nerve_hub_log=""
nerve_tmux_label=""
nerve_tmux_helper=""

nerve_fail() {
  echo "FAIL: $*" >&2
  exit 1
}

# Poll /v1/health until the hub answers, giving up if it dies or the ticks run
# out. Echoes nothing; returns non-zero so the caller can dump its own log.
nerve_wait_health() {
  local pid="$1" ticks="$2"
  for _ in $(seq 1 "$ticks"); do
    if curl -sf "$NERVE_BASE/v1/health" >/dev/null 2>&1; then
      return 0
    fi
    kill -0 "$pid" 2>/dev/null || return 1
    sleep 0.1
  done
  return 1
}

nerve_port_free() {
  ! curl -sf "$NERVE_BASE/v1/health" >/dev/null 2>&1
}

nerve_usage() {
  cat <<'EOF'
Usage: ./scripts/nerve.sh [options]

Dev launcher and verify harness. Nothing runs unless you pass a flag.

Dev options (combine as needed):
  --build              cargo build --workspace --release
  --run                build + open Nerve.app (same as --build --app)
  --app, --macos, --gui build Nerve.app, embed nerve-hub, open the app
  --tmux               cargo install nerve-hub + nerve-tmux-surface; wire ~/.tmux.conf
  --tmux-reload        tmux source-file ~/.tmux.conf
  --demo               POST fixtures/demo_snapshot.json (hub must already be up)
  --demo-surfaces      Launch seeded, real tmux + VS Code surfaces for screenshots

Verify / test (each is standalone):
  --verify-loop        ingest contract E2E (needs hub on :17890)
  --verify-surface     macOS surface regression (starts hub if needed)
  --verify-tmux        tmux surface E2E (isolated tmux server)
  --verify-vscode      VS Code surface unit tests (status / frame / focus)
  --vscode             watch the VS Code extension (F5 in a vsc-ext window)
  --test-swift         swiftc unit harness for Hub frame types
  --check-plugin       reject forbidden plugin package files
  --check-windows      compile the portable crates for Windows (no linker needed)

  node scripts/capture-vscode-surface.mjs  capture the VS Code surface screenshot

  -h, --help           show this help

Examples:
  ./scripts/nerve.sh --run
  ./scripts/nerve.sh --build --tmux --tmux-reload
  ./scripts/nerve.sh --demo
  ./scripts/nerve.sh --demo-surfaces
  ./scripts/nerve.sh --verify-loop
EOF
}

nerve_ensure_tmux_conf() {
  local conf="${HOME}/.tmux.conf"
  local entry="run-shell ${ROOT}/surfaces/tmux/nerve.tmux"
  local marker="# nerve tmux surface"

  if [[ ! -f "$conf" ]]; then
    cat >"$conf" <<EOF
${marker}
${entry}
EOF
    echo "created $conf"
    return 0
  fi

  if grep -qF 'surfaces/tmux/nerve.tmux' "$conf" 2>/dev/null; then
    echo "ok: ~/.tmux.conf already wires nerve.tmux"
    return 0
  fi

  printf '\n%s\n%s\n' "$marker" "$entry" >>"$conf"
  echo "appended nerve.tmux to $conf"
}

nerve_build_rust() {
  python3 -B "$ROOT/scripts/check-plugin.py"
  cd "$ROOT"
  cargo build --workspace --release
  echo "Built: $ROOT/target/release/nerve-hub"
}

nerve_inject_demo() {
  local base="$NERVE_BASE"
  local fixture="$NERVE_FIXTURE"

  echo "Health:"
  curl -sS "$base/v1/health"
  echo

  echo "Posting $fixture ..."
  curl -sS -X POST "$base/v1/snapshot" \
    -H 'Content-Type: application/json' \
    --data @"$fixture"
  echo

  echo "Jobs:"
  curl -sS "$base/v1/jobs" | python3 -m json.tool | head -80
}

nerve_verify_loop() {
  local base="$NERVE_BASE"

  local alias
  alias=$(python3 - <<'PY'
import socket
h = socket.gethostname() or "local"
print((h.split(".")[0] or h).strip() or "local")
PY
)

  echo "== health =="
  curl -sf "$base/v1/health" | grep -q '"ok":true'

  echo "== clear =="
  curl -sf -X POST "$base/v1/clear" >/dev/null

  echo "== inject multi-state jobs =="
  curl -sf -X POST "$base/v1/snapshot" -H 'Content-Type: application/json' -d "{
  \"alias\": \"$alias\",
  \"machineKind\": \"darwin\",
  \"jobs\": [
    {\"id\":\"a1\",\"kind\":\"session\",\"name\":\"A1\",\"alias\":\"$alias\",\"lifecycle\":\"active\",\"attention\":{\"level\":\"none\"},\"health\":\"ok\",\"progress\":{\"kind\":\"none\"},\"producer\":{\"id\":\"t\"},\"capabilities\":[],\"actions\":[],\"createdAt\":\"2026-07-19T00:00:00Z\",\"updatedAt\":\"2026-07-19T00:00:00Z\",\"version\":1,\"extensions\":{\"x\":1}},
    {\"id\":\"a2\",\"kind\":\"custom.foo\",\"name\":\"A2\",\"alias\":\"$alias\",\"lifecycle\":\"active\",\"attention\":{\"level\":\"required\",\"reason\":\"approval\"},\"health\":\"ok\",\"progress\":{\"kind\":\"none\"},\"producer\":{\"id\":\"t\"},\"capabilities\":[],\"actions\":[],\"createdAt\":\"2026-07-19T00:00:00Z\",\"updatedAt\":\"2026-07-19T00:00:00Z\",\"version\":1,\"extensions\":{}},
    {\"id\":\"a3\",\"kind\":\"build\",\"name\":\"A3\",\"alias\":\"$alias\",\"lifecycle\":\"active\",\"attention\":{\"level\":\"none\"},\"health\":\"unresponsive\",\"progress\":{\"kind\":\"none\"},\"producer\":{\"id\":\"t\"},\"capabilities\":[],\"actions\":[],\"createdAt\":\"2026-07-19T00:00:00Z\",\"updatedAt\":\"2026-07-19T00:00:00Z\",\"version\":1,\"extensions\":{}},
    {\"id\":\"a4\",\"kind\":\"test\",\"name\":\"A4\",\"alias\":\"$alias\",\"lifecycle\":\"active\",\"attention\":{\"level\":\"none\"},\"health\":\"ok\",\"progress\":{\"kind\":\"none\"},\"producer\":{\"id\":\"t\"},\"capabilities\":[],\"actions\":[],\"createdAt\":\"2026-07-19T00:00:00Z\",\"updatedAt\":\"2026-07-19T00:00:00Z\",\"version\":1,\"extensions\":{}},
    {\"id\":\"a5\",\"kind\":\"workflow\",\"name\":\"A5\",\"alias\":\"$alias\",\"lifecycle\":\"ended\",\"outcome\":\"failure\",\"attention\":{\"level\":\"informational\"},\"health\":\"ok\",\"progress\":{\"kind\":\"none\"},\"producer\":{\"id\":\"t\"},\"capabilities\":[],\"actions\":[],\"createdAt\":\"2026-07-19T00:00:00Z\",\"endedAt\":\"2026-07-19T00:01:00Z\",\"updatedAt\":\"2026-07-19T00:01:00Z\",\"version\":1,\"extensions\":{}}
  ]
}" | grep -q '"applied":5'

  echo "== event patch =="
  curl -sf -X POST "$base/v1/events" -H 'Content-Type: application/json' -d "{
  \"alias\": \"$alias\",
  \"events\":[{\"id\":\"ev1\",\"jobId\":\"a1\",\"kind\":\"attention.changed\",\"timestamp\":\"2026-07-19T00:02:00Z\",\"producerId\":\"t\",\"version\":2,\"attention\":{\"level\":\"urgent\",\"reason\":\"input\",\"title\":\"Need input\"}}]
}" | grep -q '"applied":1'

  echo "== idempotent event =="
  curl -sf -X POST "$base/v1/events" -H 'Content-Type: application/json' -d "{
  \"alias\": \"$alias\",
  \"events\":[{\"id\":\"ev1\",\"jobId\":\"a1\",\"kind\":\"attention.changed\",\"timestamp\":\"2026-07-19T00:02:00Z\",\"producerId\":\"t\",\"version\":2,\"attention\":{\"level\":\"urgent\",\"reason\":\"input\",\"title\":\"Need input\"}}]
}" | grep -q '"applied":0'

  echo "== stale version ignored =="
  curl -sf -X POST "$base/v1/events" -H 'Content-Type: application/json' -d "{
  \"alias\": \"$alias\",
  \"events\":[{\"id\":\"ev2\",\"jobId\":\"a1\",\"kind\":\"attention.changed\",\"timestamp\":\"2026-07-19T00:02:00Z\",\"producerId\":\"t\",\"version\":1,\"attention\":{\"level\":\"none\"}}]
}" | grep -q '"applied":0'

  local count
  count=$(curl -sf "$base/v1/jobs" | python3 -c "import sys,json; print(len(json.load(sys.stdin)))")
  test "$count" = "4"

  python3 - "$base" <<'PY'
import json, sys, urllib.request
data = json.load(urllib.request.urlopen(f"{sys.argv[1]}/v1/jobs"))
by_id = {s["id"]: s for s in data}
assert by_id["a1"]["attention"]["level"] == "urgent"
assert by_id["a2"]["kind"] == "custom.foo"
assert by_id["a1"]["alias"]
active = [s for s in data if s["lifecycle"] != "ended"]
assert len(active) == 4
print("active", len(active), "OK model checks")
PY

  curl -sf "$base/v1/actions/pending?producerId=t" | python3 -c "import sys,json; d=json.load(sys.stdin); assert isinstance(d,list); print('pending_ok', len(d))"

  local code
  code=$(curl -s -o /tmp/nerve_action_result.json -w "%{http_code}" -X POST "$base/v1/actions/result?producerId=missing" \
    -H 'Content-Type: application/json' -d '{"id":"nope","state":"succeeded","message":"x"}')
  test "$code" = "404"

  echo "== open alias accepted =="
  code=$(curl -s -o /tmp/nerve_unknown_alias.json -w "%{http_code}" -X POST "$base/v1/snapshot" \
    -H 'Content-Type: application/json' -d '{"alias":"__not_configured__","jobs":[]}')
  test "$code" = "200"

  echo "ALL OK"
}

nerve_verify_surface() {
  local app="$NERVE_APP"
  local hub="$app/Contents/MacOS/nerve-hub"
  local fixture="$NERVE_FIXTURE"
  local base="$NERVE_BASE"
  local grace_secs=120

  cleanup() {
    if [[ -n "$nerve_hub_pid" ]]; then
      echo "== stopping hub we started (pid $nerve_hub_pid) =="
      kill "$nerve_hub_pid" 2>/dev/null || true
      wait "$nerve_hub_pid" 2>/dev/null || true
    fi
    [[ -n "$nerve_hub_log" ]] && rm -f "$nerve_hub_log"
    return 0
  }
  trap cleanup EXIT

  echo "== bundled hub =="
  [[ -d "$app" ]] || nerve_fail "no app bundle at $app — run ./scripts/nerve.sh --run first"
  [[ -f "$hub" ]] || nerve_fail "no nerve-hub in $app/Contents/MacOS — run ./scripts/nerve.sh --run"
  [[ -x "$hub" ]] || nerve_fail "$hub is not executable"
  "$hub" --help >/dev/null 2>&1 || nerve_fail "$hub did not run"
  echo "ok: $hub"

  echo "== hub reachable on :17890 =="
  if curl -sf "$base/v1/health" >/dev/null 2>&1; then
    echo "ok: reusing the hub already holding the port"
  else
    nerve_hub_log="$(mktemp "${TMPDIR:-/tmp}/nerve-verify-surface.XXXXXX")"
    "$hub" serve >"$nerve_hub_log" 2>&1 &
    nerve_hub_pid=$!

    if ! nerve_wait_health "$nerve_hub_pid" 100; then
      echo "--- hub log ---" >&2
      cat "$nerve_hub_log" >&2 || true
      nerve_fail "bundled hub never answered /v1/health"
    fi
    echo "ok: started bundled hub (pid $nerve_hub_pid)"
  fi

  curl -sf "$base/v1/health" | grep -q '"ok":true' || nerve_fail "/v1/health did not report ok"

  echo "== fixture round trip =="
  curl -sf -X POST "$base/v1/clear" >/dev/null || nerve_fail "POST /v1/clear failed"

  curl -sf -X POST "$base/v1/snapshot" \
    -H 'Content-Type: application/json' \
    --data @"$fixture" | grep -q '"applied":2' || nerve_fail "POST /v1/snapshot did not apply the fixture's 2 jobs"

  python3 - "$base" <<'PY' || exit 1
import json, sys, urllib.request

jobs = json.load(urllib.request.urlopen(f"{sys.argv[1]}/v1/jobs"))
if len(jobs) != 2:
    print(f"FAIL: GET /v1/jobs returned {len(jobs)} job(s), expected 2", file=sys.stderr)
    sys.exit(1)

ids = sorted(j["id"] for j in jobs)
if ids != ["demo-agent-1", "demo-build-1"]:
    print(f"FAIL: GET /v1/jobs returned unexpected ids {ids}", file=sys.stderr)
    sys.exit(1)

print("ok: GET /v1/jobs ->", len(jobs), "jobs", ids)
PY

  echo "== no stray ingest-port knob =="
  local swift_scan=(-rn --include='*.swift' --exclude-dir=build)
  if grep "${swift_scan[@]}" 'ingestPort' Nerve/; then
    nerve_fail "the retired local ingest-port knob is still referenced in Nerve/"
  fi
  if grep "${swift_scan[@]}" -i 'ingestport' Nerve/ | grep -vE 'remoteIngestPort|localIngestPort'; then
    nerve_fail "unqualified ingest-port reference in Nerve/"
  fi
  echo "ok: only remoteIngestPort / localIngestPort remain"

  echo ""
  echo "ALL OK"
}

nerve_verify_tmux() {
  local plugin="$ROOT/surfaces/tmux/nerve.tmux"
  local fixture="$NERVE_FIXTURE"
  local base="$NERVE_BASE"
  nerve_tmux_label="nervesurf"
  local label="$nerve_tmux_label"
  local grace_secs="${NERVE_GRACE_SECS:-20}"
  local tick=0.1
  local ticks_health=100
  local ticks_sidebar=50

  cleanup() {
    tmux -L "$nerve_tmux_label" kill-server 2>/dev/null || true
    pkill -f "${nerve_tmux_helper:-nerve-tmux-surface}" 2>/dev/null || true
    if [[ -n "$nerve_hub_pid" ]]; then
      kill "$nerve_hub_pid" 2>/dev/null || true
      wait "$nerve_hub_pid" 2>/dev/null || true
    fi
    [[ -n "$nerve_hub_log" ]] && rm -f "$nerve_hub_log" || true
    return 0
  }
  trap cleanup EXIT

  echo "== preflight =="
  command -v tmux >/dev/null 2>&1 || nerve_fail "tmux is not installed"
  command -v curl >/dev/null 2>&1 || nerve_fail "curl is not installed"
  [[ -f "$NERVE_SURFACE_FIXTURE" ]] || nerve_fail "missing $NERVE_SURFACE_FIXTURE"
  [[ -f "$plugin" ]] || nerve_fail "no plugin entry at $plugin"
  [[ -x "$plugin" ]] || nerve_fail "$plugin is not executable"
  [[ -f "$fixture" ]] || nerve_fail "no fixture at $fixture"

  local bin_dir=""
  for candidate in "$ROOT/target/release" "$ROOT/target/debug"; do
    if [[ -x "$candidate/nerve-hub" && -x "$candidate/nerve-tmux-surface" ]]; then
      bin_dir="$candidate"
      break
    fi
  done
  [[ -n "$bin_dir" ]] || nerve_fail "no nerve-hub + nerve-tmux-surface pair in target/{release,debug} — run --build"

  local hub="$bin_dir/nerve-hub"
  nerve_tmux_helper="$bin_dir/nerve-tmux-surface"
  local helper="$nerve_tmux_helper"
  echo "ok: $bin_dir"
  export PATH="$bin_dir:$PATH"

  nerve_port_free || nerve_fail ":17890 is already serving — stop it and retry"
  tmux -L "$label" kill-server 2>/dev/null || true

  echo "== hub on :17890 (grace ${grace_secs}s) =="
  nerve_hub_log="$(mktemp "${TMPDIR:-/tmp}/nerve-verify-tmux.XXXXXX")"
  "$hub" serve >"$nerve_hub_log" 2>&1 &
  nerve_hub_pid=$!

  if ! nerve_wait_health "$nerve_hub_pid" "$ticks_health"; then
    cat "$nerve_hub_log" >&2 || true
    nerve_fail "hub never answered /v1/health"
  fi
  echo "ok: hub serving (pid $nerve_hub_pid)"

  echo "== isolated tmux server =="
  tmux -L "$label" new-session -d -s verify || nerve_fail "could not start tmux -L $label"
  local socket
  socket="$(tmux -L "$label" display-message -p '#{socket_path}')"
  [[ -n "$socket" ]] || nerve_fail "tmux -L $label did not report a socket path"
  export TMUX="${socket},0,0"
  echo "ok: $socket"

  echo "== nerve.tmux wires the surface =="
  "$plugin" || nerve_fail "nerve.tmux exited $?"

  local bind_e
  bind_e="$(tmux -L "$label" list-keys -T prefix 2>/dev/null | grep -F 'nerve-tmux-surface' | grep -F 'toggle ' || true)"
  [[ -n "$bind_e" ]] || nerve_fail "prefix + e does not run nerve-tmux-surface toggle"
  echo "ok: prefix + e -> $(tr -s ' ' <<<"$bind_e")"

  local bind_e_all
  bind_e_all="$(tmux -L "$label" list-keys -T prefix 2>/dev/null | grep -F 'toggle-all' || true)"
  [[ -z "$bind_e_all" ]] || nerve_fail "prefix + E still bound to toggle-all"

  echo "== sidebar pane opens =="
  local window_id
  window_id="$(tmux -L "$label" display-message -p '#{window_id}')"
  "$helper" toggle "$window_id" "$(pwd)" || nerve_fail "$helper toggle exited non-zero"

  local sidebar_pane=""
  for _ in $(seq 1 "$ticks_sidebar"); do
    sidebar_pane="$(tmux -L "$label" list-panes -t "$window_id" -F '#{pane_id}|#{@nerve_pane_role}' 2>/dev/null | awk -F'|' '$2=="nerve-sidebar"{print $1; exit}')"
    [[ -n "$sidebar_pane" ]] && break
    sleep "$tick"
  done
  [[ -n "$sidebar_pane" ]] || nerve_fail "toggle did not create a nerve-sidebar pane"
  echo "ok: sidebar pane $sidebar_pane"

  "$plugin" || nerve_fail "the second nerve.tmux run exited $?"
  local sidebar_again
  sidebar_again="$(tmux -L "$label" list-panes -t "$window_id" -F '#{pane_id}|#{@nerve_pane_role}' 2>/dev/null | awk -F'|' '$2=="nerve-sidebar"{print $1}' | wc -l | tr -d ' ')"
  [[ "$sidebar_again" == "1" ]] || nerve_fail "expected one sidebar pane after re-source, got $sidebar_again"
  echo "ok: one sidebar after re-source"

  echo "== injected fixture reaches the hub =="
  local inject_log
  inject_log="$(mktemp "${TMPDIR:-/tmp}/nerve-verify-tmux-inject.XXXXXX")"
  if ! nerve_inject_demo >"$inject_log" 2>&1; then
    cat "$inject_log" >&2 || true
    rm -f "$inject_log"
    nerve_fail "demo inject failed"
  fi
  rm -f "$inject_log"

  local jobs
  jobs="$(curl -sf "$base/v1/jobs")" || nerve_fail "GET /v1/jobs failed after inject"
  grep -qF 'demo-agent-1' <<<"$jobs" || nerve_fail "injected jobs missing demo-agent-1"
  grep -qF 'xcodebuild Nerve' <<<"$jobs" || nerve_fail "injected jobs missing xcodebuild Nerve"
  echo "ok: hub has demo jobs"

  echo "== hub outlives nothing: sidebar leaves, refcount drops =="
  tmux -L "$label" kill-pane -t "$sidebar_pane" 2>/dev/null || nerve_fail "could not kill sidebar pane $sidebar_pane"

  local deadline=$((grace_secs + 15))
  local gone=""
  for _ in $(seq 1 $((deadline * 10))); do
    if ! kill -0 "$nerve_hub_pid" 2>/dev/null; then
      gone=1
      break
    fi
    sleep "$tick"
  done
  if [[ -z "$gone" ]]; then
    cat "$nerve_hub_log" >&2 || true
    nerve_fail "hub was still running ${deadline}s after the last surface left"
  fi

  wait "$nerve_hub_pid" 2>/dev/null || true
  nerve_hub_pid=""
  nerve_port_free || nerve_fail ":17890 still answers after the hub exited"
  echo "ok: hub exited within grace, port released"

  echo "== no notifications, no daemons, no disk state =="
  local scan_dirs=("$ROOT/surfaces" "$ROOT/crates/nerve-tmux-surface")
  local forbidden='osascript|terminal-notifier|UNUserNotification|NSUserNotification|afplay|say -v|launchctl|systemctl'
  if grep -rnE "$forbidden" "${scan_dirs[@]}"; then
    nerve_fail "a notification or daemon call reached the tmux surface"
  fi
  local units
  units="$(find "${scan_dirs[@]}" \( -name '*.plist' -o -name '*.service' \) -print)"
  [[ -z "$units" ]] || nerve_fail "launchd/systemd unit files under the tmux surface:
$units"
  echo "ok: segment highlighting only, no unit files"

  echo ""
  echo "ALL OK"
}

nerve_test_swift() {
  cd "$ROOT"

  local swift_lang_version="5"
  local deployment_target="14.0"
  local target_triple
  target_triple="$(uname -m)-apple-macosx${deployment_target}"
  local test_file="Nerve/Tests/FrameDifferTests.swift"

  local sources=()
  while IFS= read -r f; do sources+=("$f"); done < <(find Nerve/Nerve/Models -name '*.swift' | sort)
  sources+=(
    "Nerve/Nerve/Services/Hub/HubFrame.swift"
    # Local action semantics: which machine a job runs on, and where Open goes.
    "Nerve/Nerve/Services/ActionService.swift"
    "$test_file"
  )

  [[ -f "$test_file" ]] || {
    echo "harness broken: missing $test_file" >&2
    exit 2
  }

  local present=() missing=()
  for f in "${sources[@]}"; do
    if [[ -f "$f" ]]; then
      present+=("$f")
    else
      missing+=("$f")
    fi
  done

  if ((${#missing[@]} > 0)); then
    echo "note: source(s) not implemented yet:" >&2
    printf '        %s\n' "${missing[@]}" >&2
  fi

  local build_dir
  build_dir="$(mktemp -d "${TMPDIR:-/tmp}/nerve-swift-units.XXXXXX")"
  trap 'rm -rf "$build_dir"' RETURN
  local bin="$build_dir/NerveUnitTests"

  echo "swiftc: ${#present[@]} source file(s) -> $bin"
  if ! swiftc \
    -parse-as-library \
    -swift-version "$swift_lang_version" \
    -target "$target_triple" \
    -Onone \
    -module-name NerveUnitTests \
    -o "$bin" \
    "${present[@]}"; then
    echo "RED: swiftc could not build the unit harness" >&2
    exit 1
  fi

  echo ""
  "$bin"
}

nerve_verify_vscode() {
  echo "== vscode surface unit tests =="
  [[ -f "$ROOT/vsc-ext/package.json" ]] || nerve_fail "no vsc-ext/package.json"
  if [[ ! -d "$ROOT/vsc-ext/node_modules" ]]; then
    echo "== npm install (vsc-ext) =="
    (cd "$ROOT/vsc-ext" && npm install)
  fi
  (cd "$ROOT/vsc-ext" && npm test) || nerve_fail "vsc-ext unit tests failed"
  echo "ALL OK"
}

nerve_vscode_watch() {
  echo "== vscode extension watch =="
  [[ -f "$ROOT/vsc-ext/package.json" ]] || nerve_fail "no vsc-ext/package.json"
  if [[ ! -d "$ROOT/vsc-ext/node_modules" ]]; then
    (cd "$ROOT/vsc-ext" && npm install)
  fi
  echo "Open vsc-ext in VS Code / Cursor and press F5 (Run Nerve Extension)."
  echo "Hub stays at $NERVE_BASE. This watch is the extension host bundle only."
  (cd "$ROOT/vsc-ext" && npm run watch)
}

nerve_demo_surfaces() {
  local label="nerve-demo"
  local session="nerve-demo"
  local bin_dir="$ROOT/target/debug"
  local hub="$bin_dir/nerve-hub"
  local helper="$bin_dir/nerve-tmux-surface"
  local hub_log="${TMPDIR:-/tmp}/nerve-demo-hub.log"
  local vscode_log="${TMPDIR:-/tmp}/nerve-demo-vscode.log"
  local vscode_profile="${TMPDIR:-/tmp}/nerve-demo-vscode-profile"

  echo "== build real surfaces =="
  command -v tmux >/dev/null 2>&1 || nerve_fail "tmux is not installed"
  command -v code >/dev/null 2>&1 || nerve_fail "the VS Code CLI ('code') is not installed"
  command -v curl >/dev/null 2>&1 || nerve_fail "curl is not installed"
  (cd "$ROOT" && cargo build -p nerve-hub -p nerve-tmux-surface)
  (cd "$ROOT/vsc-ext" && npm run build)
  [[ -x "$hub" && -x "$helper" ]] || nerve_fail "demo surface binaries were not built"

  echo "== local hub + demo state =="
  "$hub" ensure || nerve_fail "hub ensure failed"
  echo "ok: resident hub ready"
  curl -sf -X POST "$NERVE_BASE/v1/snapshot" \
    -H 'Content-Type: application/json' \
    --data @"$NERVE_SURFACE_FIXTURE" >/dev/null || nerve_fail "surface demo snapshot failed"
  echo "ok: seeded four stable demo jobs under alias demo-machine"

  echo "== real tmux surface =="
  tmux -L "$label" kill-server 2>/dev/null || true
  env -u NO_COLOR tmux -L "$label" new-session -d -s "$session" -x 174 -y 55 -c "$ROOT"
  local socket
  local window_id
  socket="$(tmux -L "$label" display-message -p -t "$session" '#{socket_path}')"
  window_id="$(tmux -L "$label" display-message -p -t "$session" '#{window_id}')"
  [[ -n "$socket" && -n "$window_id" ]] || nerve_fail "isolated demo tmux did not start"
  TMUX="${socket},0,0" PATH="$bin_dir:$PATH" \
    "$helper" toggle "$window_id" "$ROOT" || nerve_fail "tmux demo sidebar did not open"

  local sidebar=""
  for _ in $(seq 1 50); do
    sidebar="$(tmux -L "$label" list-panes -t "$window_id" -F '#{pane_id}|#{@nerve_pane_role}' 2>/dev/null | awk -F'|' '$2=="nerve-sidebar"{print $1; exit}')"
    [[ -n "$sidebar" ]] && break
    sleep 0.1
  done
  [[ -n "$sidebar" ]] || nerve_fail "real tmux sidebar never appeared"

  if command -v osascript >/dev/null 2>&1; then
    osascript - "$label" "$session" <<'APPLESCRIPT' >/dev/null
on run argv
  set socketLabel to item 1 of argv
  set sessionName to item 2 of argv
  tell application "Terminal"
    activate
    do script "tmux -L " & quoted form of socketLabel & " attach-session -t " & quoted form of sessionName
    set bounds of front window to {70, 70, 1510, 930}
  end tell
end run
APPLESCRIPT
  else
    echo "attach manually: tmux -L $label attach-session -t $session"
  fi
  echo "ok: tmux -L $label attach-session -t $session"

  echo "== real VS Code surface =="
  mkdir -p "$vscode_profile"
  code --user-data-dir "$vscode_profile" \
    --remote-debugging-port=9333 \
    --new-window --extensionDevelopmentPath="$ROOT/vsc-ext" "$ROOT" \
    >"$vscode_log" 2>&1 &
  sleep 2
  if command -v open >/dev/null 2>&1; then
    open -a "Visual Studio Code" "vscode://command/workbench.view.extension.nerve" \
      >/dev/null 2>&1 || true
  fi
  echo "ok: VS Code Extension Development Host launched (log $vscode_log)"
  echo "capture after it opens: node $ROOT/scripts/capture-vscode-surface.mjs"
  echo ""
  echo "Real demo is live. Capture these windows; do not redraw them for the site."
}

# --- main dispatcher ---
DO_BUILD=0
DO_APP=0
DO_TMUX=0
DO_TMUX_RELOAD=0
DO_DEMO=0
DO_DEMO_SURFACES=0
DO_VERIFY_LOOP=0
DO_VERIFY_SURFACE=0
DO_VERIFY_TMUX=0
DO_VERIFY_VSCODE=0
DO_CHECK_WINDOWS=0
DO_VSCODE=0
DO_TEST_SWIFT=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --build) DO_BUILD=1 ;;
    --run) DO_BUILD=1; DO_APP=1 ;;
    --app|--macos|--gui) DO_APP=1 ;;
    --tmux) DO_TMUX=1 ;;
    --tmux-reload) DO_TMUX_RELOAD=1 ;;
    --demo) DO_DEMO=1 ;;
    --demo-surfaces) DO_DEMO_SURFACES=1 ;;
    --verify-loop) DO_VERIFY_LOOP=1 ;;
    --verify-surface) DO_VERIFY_SURFACE=1 ;;
    --verify-tmux) DO_VERIFY_TMUX=1 ;;
    --verify-vscode) DO_VERIFY_VSCODE=1 ;;
    --check-windows) DO_CHECK_WINDOWS=1 ;;
    --vscode) DO_VSCODE=1 ;;
    --test-swift) DO_TEST_SWIFT=1 ;;
    --check-plugin) python3 -B "$ROOT/scripts/check-plugin.py"; exit $? ;;
    -h|--help) nerve_usage; exit 0 ;;
    *)
      echo "unknown option: $1" >&2
      nerve_usage >&2
      exit 2
      ;;
  esac
  shift
done

if [[ $DO_BUILD -eq 0 && $DO_APP -eq 0 && $DO_TMUX -eq 0 && $DO_TMUX_RELOAD -eq 0 && $DO_DEMO -eq 0 && $DO_DEMO_SURFACES -eq 0 \
  && $DO_VERIFY_LOOP -eq 0 && $DO_VERIFY_SURFACE -eq 0 && $DO_VERIFY_TMUX -eq 0 \
  && $DO_VERIFY_VSCODE -eq 0 && $DO_VSCODE -eq 0 && $DO_TEST_SWIFT -eq 0 \
  && $DO_CHECK_WINDOWS -eq 0 ]]; then
  nerve_usage
  exit 2
fi

if [[ $DO_BUILD -eq 1 || $DO_APP -eq 1 ]]; then
  echo "== build rust workspace =="
  nerve_build_rust
fi

if [[ $DO_APP -eq 1 ]]; then
  echo "== build + launch macOS app =="
  app="$NERVE_APP"
  cd "$ROOT/Nerve"
  xcodebuild -scheme Nerve -configuration Debug -derivedDataPath build -quiet build
  rm -f "$app/Contents/MacOS/nerve-hub"
  cp "$ROOT/target/release/nerve-hub" "$app/Contents/MacOS/nerve-hub"
  chmod +x "$app/Contents/MacOS/nerve-hub"
  codesign --force --sign - --preserve-metadata=entitlements,requirements,flags "$app"
  # `open` of a running app only activates it — the just-built binary would
  # never replace the instance already in the menu bar.
  if pgrep -x Nerve >/dev/null 2>&1; then
    killall Nerve 2>/dev/null || true
    sleep 0.4
  fi
  open "$app"
  echo "Nerve.app launched (ingest $NERVE_BASE)"
fi

if [[ $DO_TMUX -eq 1 ]]; then
  echo "== install tmux surface binaries =="
  CARGO_TARGET_DIR="$ROOT/target" cargo install --force --path "$ROOT/crates/nerve-hub"
  CARGO_TARGET_DIR="$ROOT/target" cargo install --force --path "$ROOT/crates/nerve-tmux-surface"
  nerve_ensure_tmux_conf
  echo "tmux: prefix + e toggles the sidebar"
fi

if [[ $DO_TMUX_RELOAD -eq 1 ]]; then
  if ! command -v tmux >/dev/null 2>&1; then
    echo "tmux not installed — skip reload" >&2
  elif [[ -z "${TMUX:-}" ]] && ! tmux list-sessions >/dev/null 2>&1; then
    echo "no tmux server — start tmux, then: tmux source-file ~/.tmux.conf"
  else
    tmux source-file "${HOME}/.tmux.conf"
    echo "ok: tmux config reloaded"
  fi
fi

if [[ $DO_DEMO -eq 1 ]]; then
  echo "== inject demo fixture =="
  nerve_inject_demo
fi

if [[ $DO_DEMO_SURFACES -eq 1 ]]; then
  nerve_demo_surfaces
fi

if [[ $DO_VERIFY_LOOP -eq 1 ]]; then
  nerve_verify_loop
fi

if [[ $DO_VERIFY_SURFACE -eq 1 ]]; then
  nerve_verify_surface
fi

if [[ $DO_VERIFY_TMUX -eq 1 ]]; then
  nerve_verify_tmux
fi

nerve_check_windows() {
  # `cargo check` does not link, so this needs only the target's standard
  # library — no Windows machine, no linker, no SDK. It is what stands between
  # the Windows surfaces and a `cfg(windows)` arm that was never compiled,
  # because a green `cargo test` on macOS says nothing about one.
  #
  # nerve-tmux-surface is absent on purpose: tmux has no Windows port.
  echo "==> cargo check (windows target)"
  if ! rustup target list --installed | grep -q '^x86_64-pc-windows-msvc$'; then
    echo "    adding the target"
    rustup target add x86_64-pc-windows-msvc
  fi
  cargo check \
    -p nerve-platform -p nerve-hub -p nerve-surface-core -p nerve-windows-surface \
    --all-targets --all-features --target x86_64-pc-windows-msvc
  echo "ALL OK"
}

if [[ $DO_CHECK_WINDOWS -eq 1 ]]; then
  nerve_check_windows
fi

if [[ $DO_VERIFY_VSCODE -eq 1 ]]; then
  nerve_verify_vscode
fi

if [[ $DO_VSCODE -eq 1 ]]; then
  nerve_vscode_watch
fi

if [[ $DO_TEST_SWIFT -eq 1 ]]; then
  nerve_test_swift
fi

echo "done"
