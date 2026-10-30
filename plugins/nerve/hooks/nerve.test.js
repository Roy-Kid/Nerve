"use strict";
const { test } = require("node:test");
const assert = require("node:assert/strict");
const http = require("node:http");
const { spawnSync } = require("node:child_process");
const { forward, enrich, walkAgent } = require("./nerve.js");
test("raw structured event is forwarded to hub without a task snapshot", async () => {
 let resolve; const received = new Promise(r => resolve = r);
 const server = http.createServer((request,response)=>{let body="";request.on("data",chunk=>body+=chunk);request.on("end",()=>{resolve({path:request.url,body:JSON.parse(body)});response.end("{}");});});
 await new Promise(r=>server.listen(0,"127.0.0.1",r));
 try {
  await forward({hook_event_name:"Stop",session_id:"s",background_tasks:[{type:"shell",name:"tests"}]},"claude",server.address().port);
  const request=await received;
  assert.equal(request.path,"/v1/hook?producer=claude");
  assert.equal(request.body.session_id,"s");assert.deepEqual(request.body.background_tasks,[{type:"shell",name:"tests"}]);
  assert.equal(request.body.jobs,undefined);assert.equal(request.body.lifecycle,undefined);
 } finally {await new Promise(r=>server.close(r));}
});
test("slot is stable and source payload is not mutated",()=>{const input={session_id:"a"};assert.equal(enrich(input).slotId,enrich({session_id:"b"}).slotId);assert.deepEqual(input,{session_id:"a"});});
test("agent pid stops at node or python and only walks through a shell", () => {
  const tree = { 4: "3 bash", 3: "2 node", 2: "1 claude" };
  assert.equal(walkAgent(4, pid => tree[pid] || ""), 3);
  assert.equal(walkAgent(3, pid => tree[pid] || ""), 3);
  assert.equal(walkAgent(8, pid => "7 python3"), 8);
  assert.equal(walkAgent(6, pid => "5 run.sh"), 5);
});
test("invalid input is fail-open and silent",()=>{for(const input of ["", "not json", "[]"]) {const out=spawnSync(process.execPath,[__dirname+"/nerve.js"],{input,timeout:2000});assert.equal(out.status,0);assert.equal(out.stdout.length,0);}});
test('a hook with no tty of its own reports an ancestor terminal', {skip:process.platform !== 'darwin'}, () => {
 const inner = `process.stdout.write(JSON.stringify(require(${JSON.stringify(__dirname + '/nerve.js')}).enrich({})))`;
 const outer = `const {spawnSync}=require('node:child_process');const out=spawnSync(process.execPath,['-e',${JSON.stringify(inner)}],{encoding:'utf8',stdio:['ignore','pipe','pipe'],detached:true});process.stdout.write(out.stdout||'');`;
 const out = spawnSync('script',['-q','/dev/null',process.execPath,'-e',outer],{encoding:'utf8',timeout:4000,stdio:['ignore','pipe','pipe']});
 assert.equal(out.status,0,out.stderr);
 const event = JSON.parse(out.stdout.match(/\{[^\r\n]+\}/)[0]);
 assert.match(event.terminalName,/^\/dev\/(ttys\d+|pts\/\d+)$/);
 assert.equal(event.terminalHost,require('node:os').hostname().toLowerCase());
});
test('a piped hook still reports its controlling terminal without relying on agent PID discovery', {skip:process.platform !== 'darwin'}, () => {
 const program = `process.stdout.write(JSON.stringify(require(${JSON.stringify(__dirname + '/nerve.js')}).enrich({})))`;
 const out = spawnSync('script',['-q','/dev/null',process.execPath,'-e',program],{encoding:'utf8',timeout:2000,stdio:['ignore','pipe','pipe']});
 assert.equal(out.status,0);
 const event = JSON.parse(out.stdout.match(/\{[^\r\n]+\}/)[0]);
 assert.match(event.terminalName,/^\/dev\/ttys\d+$/);
 assert.equal(event.terminalHost,require('node:os').hostname().toLowerCase());
});
