import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {statusOf,visibleJobs,readFrame} from '../package/web/model.mjs';

test('frames replace the live list and never resurrect departed sessions',()=>{
  assert.deepEqual(readFrame('{"jobs":[],"departed":[{"id":"ended"}]}'),[]);
  assert.throws(()=>readFrame('{"jobs":[null]}'));
  assert.throws(()=>readFrame('{"jobs":"wrong"}'));
});
test('machine filtering is explicit and cannot silently fall back to all machines',()=>{
  const jobs=[{id:'a',name:'中文终端',alias:'cluster',lifecycle:'active'},{id:'b',alias:'local',lifecycle:'active'}];
  assert.equal(visibleJobs(jobs,{query:'中文'}).length,1);
  assert.deepEqual(visibleJobs(jobs,{host:'missing',scoped:true}),[]);
  assert.equal(visibleJobs(jobs,{host:'CLUSTER',scoped:true})[0].id,'a');
  assert.equal(visibleJobs(jobs).length,2);
});
