"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const { mapEvent, eventName, sessionId } = require("./nerve.js");

test("SessionStart is Ready, not Running", () => {
  const f = mapEvent("sessionstart", { hook_event_name: "SessionStart" });
  assert.equal(f.current.type, "starting");
  assert.equal(f.attention.level, "none");
});

test("Stop completes the turn without asking for input", () => {
  const f = mapEvent("stop", { hookEventName: "Stop" });
  assert.equal(f.current.type, "completed");
  assert.equal(f.attention.level, "none");
  assert.equal(f.lifecycle, "active");
  assert.equal(f.outcome, undefined);
});

test("Stop while the gate is retrying stays running", () => {
  const f = mapEvent("stop", { hook_event_name: "Stop", stopHookActive: true });
  assert.equal(f.current.type, "thinking");
  assert.equal(f.attention.level, "none");
});

test("session-end observe Stop does not paint your turn", () => {
  assert.equal(mapEvent("stop", { hook_event_name: "Stop", reason: "channel_closed" }), null);
});

test("Stop with shell background is Running", () => {
  const f = mapEvent("stop", {
    hook_event_name: "Stop",
    background_tasks: [{ type: "shell", description: "npm test" }],
  });
  assert.equal(f.current.type, "subagent");
  assert.equal(f.attention.level, "none");
});

test("PreCompact with shell background stays Running", () => {
  const f = mapEvent("precompact", {
    hook_event_name: "PreCompact",
    background_tasks: [{ type: "shell", description: "npm test" }],
  });
  assert.equal(f.current.type, "subagent");
  assert.equal(f.attention.level, "none");
});

test("PreCompact without background is thinking", () => {
  const f = mapEvent("precompact", { hook_event_name: "PreCompact" });
  assert.equal(f.current.type, "thinking");
  assert.equal(f.current.summary, "Compacting context");
  assert.equal(f.attention.level, "none");
});

test("PostCompact continues working", () => {
  const f = mapEvent("postcompact", { hook_event_name: "PostCompact" });
  assert.equal(f.current.type, "thinking");
  assert.equal(f.attention.level, "none");
});

test("PostCompact with monitor stays monitor", () => {
  const f = mapEvent("postcompact", {
    hook_event_name: "PostCompact",
    background_tasks: [{ type: "monitor" }],
  });
  assert.equal(f.current.type, "monitor");
  assert.equal(f.outcome, "partial");
});

test("event name strips punctuation", () => {
  assert.equal(eventName({ hook_event_name: "User_Prompt-Submit" }), "userpromptsubmit");
});

test("session id prefers session_id", () => {
  assert.equal(sessionId({ session_id: "abc" }), "abc");
});

// ── Wire paths ──────────────────────────────────────────────────────────────

const { fileUri, pathStyle, buildLocation } = require("./nerve.js");

test("a posix cwd encodes exactly as it always did", () => {
  assert.equal(fileUri("/Users/me/proj"), "file:///Users/me/proj");
  assert.equal(fileUri("/Users/me/proj foo"), "file:///Users/me/proj%20foo");
});

test("a windows cwd gains the RFC slash and keeps its colon", () => {
  assert.equal(fileUri("C:\\Users\\me\\work"), "file:///C:/Users/me/work");
  assert.equal(fileUri("c:/Users/me/work"), "file:///c:/Users/me/work");
});

test("a unc server becomes the url authority", () => {
  assert.equal(fileUri("\\\\srv\\share\\proj"), "file://srv/share/proj");
});

test("a relative cwd gets no url", () => {
  // `C:work` is relative to the current directory on drive C.
  for (const cwd of ["proj", "~/proj", "C:work", "", null]) {
    assert.equal(fileUri(cwd), null, String(cwd));
  }
});

test("path style knows both OSes", () => {
  assert.equal(pathStyle("/tmp"), "posix");
  assert.equal(pathStyle("C:\\tmp"), "windows");
  assert.equal(pathStyle("\\\\srv\\share"), "windows");
  assert.equal(pathStyle("tmp"), null);
});

test("an ide deep link carries a drive after the url's slash", () => {
  const prev = process.env.VSCODE_PID;
  process.env.VSCODE_PID = "1";
  try {
    assert.equal(
      buildLocation({}, "C:\\work\\nerve").openURL,
      "vscode://file/C:/work/nerve",
    );
    assert.equal(
      buildLocation({}, "/Users/me/nerve").openURL,
      "vscode://file/Users/me/nerve",
    );
  } finally {
    if (prev === undefined) delete process.env.VSCODE_PID;
    else process.env.VSCODE_PID = prev;
  }
});

test("a windows cwd still reports a focus hint with the path in it", () => {
  const loc = buildLocation({}, "C:\\work\\nerve");
  assert.ok(loc.focusHint.endsWith(" · C:\\work\\nerve"));
});

// ── Transport ───────────────────────────────────────────────────────────────

const http = require("node:http");
const { postSnapshot, postIngest } = require("./nerve.js");

/** A throwaway ingest server on an ephemeral port. Never 17890. */
function waitUntil(pred, ms = 500) {
  const started = Date.now();
  return new Promise((resolve) => {
    const tick = () => {
      if (pred() || Date.now() - started > ms) resolve();
      else setTimeout(tick, 10);
    };
    tick();
  });
}

function ingest(handler) {
  return new Promise((resolve) => {
    const server = http.createServer((req, res) => {
      let body = "";
      req.on("data", (chunk) => { body += chunk; });
      req.on("end", () => {
        res.writeHead(200, { "Content-Type": "application/json" });
        res.end("{}");
        handler({ method: req.method, url: req.url, headers: req.headers, body });
      });
    });
    server.listen(0, "127.0.0.1", () => resolve(server));
  });
}

test("a snapshot arrives as a well-formed POST", async () => {
  let seen;
  const server = await ingest((request) => { seen = request; });
  const { port } = server.address();
  assert.notEqual(port, 17890, "a test must never take the hub's own port");

  try {
    await postSnapshot("thinkpad", "windows", { id: "j", name: "nerve" }, port);
    // The post resolves when the bytes are flushed, which is before this
    // process has parsed them as an incoming request.
    await waitUntil(() => seen);
  } finally {
    server.close();
  }

  assert.equal(seen.method, "POST");
  assert.equal(seen.url, "/v1/snapshot");
  assert.equal(seen.headers["content-type"], "application/json");
  const payload = JSON.parse(seen.body);
  assert.equal(payload.alias, "thinkpad");
  assert.equal(payload.machineKind, "windows");
  assert.deepEqual(payload.jobs, [{ id: "j", name: "nerve" }]);
});

test("a multibyte job name keeps its Content-Length honest", async () => {
  // The hand-written request used `body.length` on a Buffer, which is bytes —
  // correct, but only by luck of the author remembering. Pin it.
  let seen;
  const server = await ingest((request) => { seen = request; });
  const { port } = server.address();
  try {
    await postSnapshot("mac", "darwin", { id: "j", name: "项目" }, port);
    await waitUntil(() => seen);
  } finally {
    server.close();
  }
  const expected = Buffer.byteLength(seen.body);
  assert.equal(Number(seen.headers["content-length"]), expected);
  assert.equal(JSON.parse(seen.body).jobs[0].name, "项目");
});

test("nothing listening resolves rather than throwing", async () => {
  // Fail-open: no hook event may ever block or crash an agent.
  const server = await ingest(() => {});
  const { port } = server.address();
  server.close();
  await new Promise((r) => server.on("close", r));

  const started = Date.now();
  await postSnapshot("mac", "darwin", { id: "j", name: "nerve" }, port);
  assert.ok(Date.now() - started < 500);
});

test("a hub that never answers does not hold the post", async () => {
  let seen;
  const server = await new Promise((resolve) => {
    const hanging = http.createServer((req) => {
      let body = "";
      req.on("data", (chunk) => { body += chunk; });
      req.on("end", () => { seen = body; });
      // Deliberately no response.
    });
    hanging.listen(0, "127.0.0.1", () => resolve(hanging));
  });
  const { port } = server.address();
  const started = Date.now();
  try {
    await postIngest(port, "/v1/hook?producer=grok", Buffer.from('{"ok":true}'));
    assert.ok(Date.now() - started < 500);
    await waitUntil(() => seen !== undefined);
  } finally {
    server.close();
  }
  assert.equal(seen, '{"ok":true}');
});

test("Claude hooks run in the background and do not show a Nerve spinner", () => {
  // async: the agent continues without waiting for the process. No
  // statusMessage: the turn must not advertise that Nerve is in the path.
  const claude = require("./hooks.json");
  for (const event of Object.keys(claude.hooks)) {
    for (const group of claude.hooks[event]) {
      for (const hook of group.hooks) {
        // SessionEnd stays synchronous: hosts tear async work down when the
        // session exits, and the end snapshot has to be written first. The
        // script still returns without reading the hub's response.
        if (event === "SessionEnd") assert.equal(hook.async, undefined, event);
        else assert.equal(hook.async, true, event);
        assert.equal(hook.statusMessage, undefined, event);
      }
    }
  }
});

test("Grok hooks are command posts, not type:http to loopback", () => {
  // Grok's HTTP runner blocks 127.0.0.1 (SSRF) and non-HTTPS URLs, so a
  // type:http hook never reaches the hub.
  const grok = require("./grok.json");
  const events = Object.keys(grok.hooks);
  assert.ok(events.includes("PreToolUse"));
  assert.ok(events.includes("SessionStart"));
  for (const event of events) {
    for (const group of grok.hooks[event]) {
      for (const hook of group.hooks) {
        assert.equal(hook.type, "command", event);
        assert.match(hook.command, /grok-post\.js/);
        assert.equal(hook.timeout, 2);
      }
    }
  }
});

test("grok-post.js exits 0 with empty stdin", async () => {
  const { spawn } = require("child_process");
  const child = spawn(process.execPath, [require("path").join(__dirname, "grok-post.js")], {
    stdio: ["pipe", "ignore", "ignore"],
  });
  child.stdin.end();
  const code = await new Promise((resolve) => child.on("close", resolve));
  assert.equal(code, 0);
});

test("grok-post.js exits before a silent hub answers", async () => {
  const { spawn } = require("child_process");
  const os = require("os");
  const fs = require("fs");
  // A warm identity cache so the assertion measures the POST, not the ps climb.
  const token = "TERM_SESSION_ID:grok-fire-forget-test";
  const idFile = require("path").join(os.tmpdir(), "nerve-hook", `identity-${token.replace(/[^\w.-]/g, "_")}.json`);
  fs.mkdirSync(require("path").dirname(idFile), { recursive: true });
  fs.writeFileSync(idFile, JSON.stringify({ alias: "test", pid: 123, ts: new Date().toISOString() }));
  let seen = "";
  const server = await new Promise((resolve) => {
    const hanging = http.createServer((req) => {
      req.on("data", (chunk) => { seen += chunk; });
    });
    hanging.listen(0, "127.0.0.1", () => resolve(hanging));
  });
  const { port } = server.address();
  const child = spawn(process.execPath, [
    require("path").join(__dirname, "grok-post.js"),
    "--port",
    String(port),
  ], {
    stdio: ["pipe", "ignore", "ignore"],
    env: { ...process.env, TERM_SESSION_ID: "grok-fire-forget-test" },
  });
  child.stdin.end(JSON.stringify({
    hook_event_name: "PreToolUse",
    session_id: "s",
    cwd: "/tmp",
  }));
  const started = Date.now();
  const code = await new Promise((resolve) => child.on("close", resolve));
  try {
    assert.equal(code, 0);
    assert.ok(Date.now() - started < 500);
    await waitUntil(() => seen.includes("PreToolUse"));
    assert.ok(seen.includes("PreToolUse"));
  } finally {
    server.close();
  }
});

for (const notification_type of ["agent_needs_input", "elicitation_dialog", "permission_prompt"]) {
  test(`${notification_type} still requires the human after completion`, () => {
    const done = mapEvent("stop", {});
    assert.equal(done.current.type, "completed");
    const ask = mapEvent("notification", { notification_type });
    assert.notEqual(ask.attention.level, "none");
    assert.equal(mapEvent("notification", { notification_type: "idle_prompt", message: "Needs approval" }), null);
    assert.equal(mapEvent("userpromptsubmit", { prompt: "Continue" }).attention.level, "none");
  });
}

// ── Phase 2/3 parity: the strings and shapes all three mappers must agree on ─

const { localActions } = require("./nerve.js");

test("permissiondenied is the same ask as permissionrequest", () => {
  const a = mapEvent("permissiondenied", { tool_name: "Bash", tool_input: { command: "ls" } });
  const b = mapEvent("permissionrequest", { tool_name: "Bash", tool_input: { command: "ls" } });
  assert.equal(a.current.type, "waiting");
  assert.equal(a.attention.level, "required");
  assert.equal(a.attention.reason, "approval");
  assert.equal(a.attention.title, "Approval needed in agent");
  assert.equal(a.attention.summary, 'Bash: {"command":"ls"}');
  assert.deepEqual(a, b);
});

test("stopcancelled and elicitation both ask for the human", () => {
  for (const event of ["stopcancelled", "elicitation"]) {
    const f = mapEvent(event, {});
    assert.equal(f.current.type, "idle", event);
    assert.equal(f.attention.level, "suggested", event);
    assert.equal(f.attention.reason, "input", event);
    assert.equal(f.attention.title, "Your turn in agent", event);
  }
});

test("postcompact keeps working on its own copy", () => {
  assert.equal(mapEvent("postcompact", {}).current.summary, "Context compacted — continuing");
  assert.equal(mapEvent("precompact", {}).current.summary, "Compacting context");
});

test("posttoolusefailure titles the tool; stopfailure says turn", () => {
  assert.equal(mapEvent("posttoolusefailure", { tool_name: "Bash" }).attention.title, "Bash failed");
  assert.equal(mapEvent("stopfailure", { error: "boom" }).attention.title, "Turn failed");
  assert.equal(mapEvent("stopfailure", { error: "boom" }).current.summary, "boom");
});

test("all four subagent tools report background on posttooluse", () => {
  for (const tool of ["spawn_subagent", "get_command_or_subagent_output", "Task", "Agent"]) {
    const f = mapEvent("posttooluse", {
      tool_name: tool,
      tool_response: { status: "async_launched", description: "explore" },
    });
    assert.equal(f.current.type, "subagent", tool);
    assert.equal(f.current.summary, "Background: explore", tool);
  }
});

test("background summary lists every task and labels the mix", () => {
  const mixed = mapEvent("stop", {
    background_tasks: [
      { type: "shell", description: "npm test" },
      { type: "monitor" },
    ],
  });
  assert.equal(mixed.current.name, "mixed");
  assert.equal(mixed.current.summary, "2 background task(s): npm test, monitor");

  const only = mapEvent("stop", { background_tasks: [{ type: "monitor" }] });
  assert.equal(only.current.name, "monitor");
  assert.equal(only.current.summary, "1 background task(s): monitor");
});

test("task and teammate events are info, never attention", () => {
  const created = mapEvent("taskcreated", { title: "write tests" });
  assert.equal(created.current.type, "info");
  assert.equal(created.current.name, "write tests");
  assert.equal(created.current.summary, "Task created: write tests");
  assert.equal(created.attention.level, "none");

  const idle = mapEvent("teammateidle", { agent_type: "Explore" });
  assert.equal(idle.current.summary, "Teammate idle: Explore");
  assert.equal(idle.attention.level, "none");

  assert.equal(mapEvent("cwdchanged", {}).current.summary, "Workspace changed");
  assert.equal(mapEvent("elicitationresult", { title: "Answered" }).current.summary, "Answered");
});

test("silent-noop events never paint a facet", () => {
  for (const event of [
    "setup", "userpromptexpansion", "posttoolbatch", "messagedisplay",
    "instructionsloaded", "configchange", "directoryadded", "filechanged",
  ]) {
    assert.equal(mapEvent(event, {}), null, event);
  }
});

// ── job body: actions, location, extensions ─────────────────────────────────

test("actions fall back to Focus when only a focusHint exists", () => {
  const withUrl = localActions({ openURL: "file:///tmp/p", focusHint: "x" });
  assert.equal(withUrl[0].id, "open");
  assert.equal(withUrl[0].kind, "open");
  assert.equal(withUrl[0].title, "Open");

  const hintOnly = localActions({ focusHint: "x" });
  assert.equal(hintOnly[0].id, "open");
  assert.equal(hintOnly[0].kind, "focus");
  assert.equal(hintOnly[0].title, "Focus");

  const neither = localActions({});
  assert.equal(neither.length, 1);
  assert.equal(neither[0].kind, "copy_summary");
});

test("open_logs rides along only when a logPath is present", () => {
  const withLog = localActions({ openURL: "file:///tmp/p", logPath: "/tmp/t.jsonl" });
  assert.deepEqual(withLog.map((a) => a.kind), ["open", "copy_summary", "open_logs"]);

  const without = localActions({ openURL: "file:///tmp/p" });
  assert.deepEqual(without.map((a) => a.kind), ["open", "copy_summary"]);
});

test("the focus breadcrumb names the hosting terminal", () => {
  const prev = process.env.TERM_SESSION_ID;
  process.env.TERM_SESSION_ID = "abc";
  try {
    assert.ok(buildLocation({}, "/tmp/p").focusHint.includes(" · Terminal · "));
  } finally {
    if (prev === undefined) delete process.env.TERM_SESSION_ID;
    else process.env.TERM_SESSION_ID = prev;
  }
});
