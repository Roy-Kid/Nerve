const { test } = require('node:test');
const assert = require('node:assert/strict');
const model = require('../../surfaces/tether/web/model.js');
const frame = require('../../fixtures/hub-frame.json');
test('reads authoritative projection, order and summary', () => {
  const rows=model.jobs(frame);
  assert.deepEqual(rows.map(j=>j.id),frame.jobs.map(j=>j.id));
  for(const row of rows) assert.equal(model.classify({...row,lifecycle:'ended',outcome:'failure'}),row.display.status);
  assert.match(model.status(rows,true,frame.summary).label,new RegExp(`${frame.summary.total} jobs`));
  assert.deepEqual(model.status(rows,false,frame.summary).segments,[]);
});
test('rejects an old protocol instead of managing jobs locally', () => {
  assert.throws(()=>model.jobs({jobs:[]}),/Upgrade/);
});
test('filters canonical statuses independently and preserves authoritative order', () => {
  const states = model.order.map((status, index) => ({
    id: String(index), name: status, display: { status, machineLabel: 'studio' }
  }));
  const rows = model.jobs({protocolVersion:2,hubInstanceId:'hub',revision:1,jobs:states});
  for (const status of model.order) assert.deepEqual(model.select(rows,status).map(job=>job.status),[status]);
  assert.deepEqual(model.select(rows,'all').map(job=>job.id),states.map(job=>job.id));
});
test('search intersects the filter across display, producer and location without status inference', () => {
  const rows = model.jobs(frame);
  assert.deepEqual(model.select(rows,'running','  NERVE ').map(job=>job.id),['demo-2','demo-1']);
  assert.deepEqual(model.select(rows,'attention','xcode'),[]);
  assert.equal(model.select(rows,'all','fixture-machine').length,rows.length);
  assert.equal(model.select(rows,'all','demo').length,rows.length);
  assert.deepEqual(model.select(rows,'all','/tmp').map(job=>job.id),['demo-3','demo-1']);
  assert.deepEqual(model.select(rows,'all','NO HEARTBEAT'),[], 'search follows display.activity, not hidden status-like text');
});
test('rejects malformed snapshots instead of replacing valid state with an empty list', () => {
  for (const patch of [{jobs:null},{jobs:[null]},{jobs:[{id:'bad'}]},
    {jobs:[{id:'bad',display:{status:'unknown'}}]}, {revision:NaN}, {revision:-1}, {hubInstanceId:null},
    {machines:[null]}, {jobs:[frame.jobs[0],frame.jobs[0]]},
    {jobs:[{...frame.jobs[0],timeline:[null]}]}]) {
    assert.throws(()=>model.jobs({...frame,...patch}),/Invalid/);
  }
});
test('tab marks translate hub-selected states without classifying or deduplicating jobs', () => {
  const target = {host:'studio',terminal:'/dev/ttys001'};
  assert.deepEqual(model.tabIndicators({tabIndicators:[{target,status:'attention',label:'Needs you'}]}),
    [{target,color:model.colors[1],label:'Needs you'}]);
  assert.deepEqual(model.tabIndicators({}),[]);
  assert.deepEqual(model.tabIndicators({tabIndicators:[{target,status:'unknown'}]}),[]);
});
