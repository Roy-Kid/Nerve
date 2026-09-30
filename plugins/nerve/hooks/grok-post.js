#!/usr/bin/env node
/**
 * Grok command hook: stdin event JSON → POST http://127.0.0.1:17890/v1/hook?producer=grok
 *
 * Grok's `type: http` runner refuses loopback and non-HTTPS URLs (SSRF guard),
 * so the official HTTP hook type cannot reach the hub. Grok also waits for
 * this process to exit, and it has no async hook field — so this writes the
 * POST and exits without reading the response. Always exit 0. Nothing on
 * stdout: Grok would treat it as a PreToolUse decision.
 */
"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");

const INGEST_PORT = 17890;

// Same algorithm as nerve.js, so a Grok row can supersede ghosts and be
// PID-reaped exactly like a Claude one. Required, not copied — one climb.
// `postIngest` writes the body and returns without reading the hub's answer.
const { agentPid, slotId, postIngest } = require("./nerve.js");

function note(message) {
  try {
    const dir = path.join(os.homedir(), "Library/Logs/Nerve");
    fs.mkdirSync(dir, { recursive: true });
    fs.appendFileSync(path.join(dir, "nerve-hook.log"), `${new Date().toISOString()} grok ${message}\n`);
  } catch (_) {
    /* fail-open */
  }
}

function finish() {
  process.exit(0);
}

const chunks = [];
process.stdin.on("data", (chunk) => {
  chunks.push(chunk);
});
process.stdin.on("error", finish);
process.stdin.on("end", () => {
  const raw = Buffer.concat(chunks);
  if (!raw.length) {
    note("empty stdin");
    finish();
    return;
  }
  // Inject slot + pid at the top level so `hook::build` can pick them up and
  // publish `extensions.slot` / `extensions.pid`. Fail-open: a body we cannot
  // parse is forwarded unchanged rather than dropped.
  let body = raw;
  try {
    const payload = JSON.parse(raw.toString("utf8"));
    if (payload && typeof payload === "object" && !Array.isArray(payload)) {
      const pid = agentPid();
      if (pid) payload.agentPid = pid;
      payload.slotId = slotId(payload);
      body = Buffer.from(JSON.stringify(payload));
    }
  } catch (_) {
    body = raw;
  }
  const portFlag = process.argv.indexOf("--port");
  const port = portFlag > 0 && process.argv[portFlag + 1]
    ? Number(process.argv[portFlag + 1])
    : INGEST_PORT;
  postIngest(port, "/v1/hook?producer=grok", body).then(finish, (error) => {
    note(`error ${error && error.message ? error.message : error}`);
    finish();
  });
});
