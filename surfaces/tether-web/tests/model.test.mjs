import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {statusOf,visibleJobs,readFrame} from '../package/web/model.mjs';

test('structured status agrees with the shared surface priority ladder',()=>{
  const cases = [
    [{lifecycle:'ended',outcome:'failure'},'problem'],
    [{lifecycle:'ended',outcome:'success',health:'unresponsive'},'problem'],
    [{lifecycle:'active',attention:{reason:'input',level:'required'}},'attention'],
    [{lifecycle:'active',attention:{reason:'dependency',level:'urgent'}},'waiting'],
    [{lifecycle:'active',current:{type:'tool'},attention:{reason:'approval',level:'required'}},'running'],
    [{lifecycle:'active',current:{type:'completed'},attention:{level:'none'}},'success'],
    [{lifecycle:'active',outcome:'partial'},'monitor'],
    [{lifecycle:'active',current:{type:'monitor'}},'monitor'],
    [{lifecycle:'active',current:{type:'idle'}},'inactive'],
    [{lifecycle:'pending'},'waiting'],
    [{lifecycle:'active',health:'degraded'},'problem'],
    [{lifecycle:'ended',outcome:'success',attention:{reason:'approval',level:'urgent'}},'success'],
    [{lifecycle:'active',current:{summary:'ERROR! Please approve this!'}},'running'],
  ];
  for(const [job,status] of cases)assert.equal(statusOf(job),status,JSON.stringify(job));
});
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
test('the current hub demo fixture renders every reported conversation once',()=>{
  const fixture=JSON.parse(readFileSync(new URL('../../../fixtures/surface_demo_snapshot.json',import.meta.url)));
  const jobs=readFrame(JSON.stringify(fixture));
  assert.equal(visibleJobs(jobs).length,jobs.length);
  assert.equal(new Set(jobs.map(job=>job.id)).size,jobs.length);
  for(const job of jobs)assert.ok(['problem','attention','waiting','running','monitor','success','inactive'].includes(statusOf(job)));
});
