#!/usr/bin/env node
"use strict";
const { forward } = require("./nerve.js");
const chunks=[];
process.stdin.on("data", chunk => chunks.push(chunk));
process.stdin.on("error", () => process.exit(0));
process.stdin.on("end", async () => {
  try { await forward(JSON.parse(Buffer.concat(chunks).toString("utf8")), "grok"); } catch (_) {}
  process.exit(0);
});
