#!/usr/bin/env node
// Pure host adapter: hub owns all task and session semantics.
"use strict";
const http = require("node:http");
const INGEST_HOST = "127.0.0.1";
const INGEST_TIMEOUT_MS = 250;
const crypto = require("node:crypto");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const SLOT_ENV_KEYS = ["TERM_SESSION_ID","ITERM_SESSION_ID","WEZTERM_PANE","KITTY_WINDOW_ID","TMUX_PANE","WT_SESSION","ConEmuPID"];
let cachedPid;
function agentPid() {
  if (cachedPid !== undefined) return cachedPid;
  let pid = process.ppid;
  if (process.platform === "win32") return cachedPid = pid;
  for (let i=0; i<6 && pid>1; i++) {
    const result = spawnSync("ps", ["-p", String(pid), "-o", "ppid=,comm="], {encoding:"utf8", timeout:100, stdio:["ignore","pipe","ignore"]});
    const line = (result.stdout || "").trim();
    const match = line.match(/^(\d+)\s+(.+)$/);
    if (!match) break;
    const name = path.basename(match[2]).toLowerCase();
    if (!["bash","sh","zsh","fish","dash","node","python3","python"].includes(name) && !name.endsWith(".sh")) break;
    const parent = Number(match[1]);
    if (parent <= 1 || parent === pid) break;
    pid = parent;
  }
  return cachedPid = pid > 1 ? pid : null;
}
function slotId() {
  const key = SLOT_ENV_KEYS.find(key => process.env[key]);
  const token = key ? `${key}:${process.env[key]}` : `pid:${agentPid()}`;
  return crypto.createHash("sha256").update(token).digest("hex").slice(0,32);
}
function postIngest(port, urlPath, body) {
  const payload = Buffer.isBuffer(body) ? body : Buffer.from(body);
  return new Promise((resolve) => {
    let done = false;
    let timer;
    const finish = () => {
      if (done) return;
      done = true;
      clearTimeout(timer);
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
    request.on("response", response => { response.resume(); finish(); });
    timer = setTimeout(() => { request.destroy(); finish(); }, INGEST_TIMEOUT_MS);
    request.end(payload);

  });
}

function enrich(payload) {
  const result = { ...payload, agentPid: agentPid(), slotId: slotId() };
  // Raw terminal identity only. State and per-terminal priority belong to hub.
  if (process.platform !== 'win32') {
    const tty = spawnSync('ps', ['-p', String(process.pid), '-o', 'tty='],
      {encoding:'utf8', timeout:100, stdio:['ignore','pipe','ignore']});
    const name = (tty.stdout || '').trim().replace(/^\/dev\//, '');
    if (/^(ttys\d+|pts\/\d+)$/.test(name)) {
      result.terminalHost = require('node:os').hostname().toLowerCase();
      result.terminalName = '/dev/' + name;
    }
  }
  return result;
}
async function forward(payload, producer="claude", port=17890) {
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) return;
  await postIngest(port, `/v1/hook?producer=${encodeURIComponent(producer)}`, Buffer.from(JSON.stringify(enrich(payload))));
}
if (require.main === module) {
  const chunks=[];
  process.stdin.on("data", chunk => chunks.push(chunk));
  process.stdin.on("error", () => process.exit(0));
  process.stdin.on("end", async () => {
    try { await forward(JSON.parse(Buffer.concat(chunks).toString("utf8"))); } catch (_) {}
    process.exit(0);
  });
}
module.exports = { agentPid, slotId, postIngest, enrich, forward };
