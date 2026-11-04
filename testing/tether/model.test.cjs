const { test } = require('node:test');
const assert = require('node:assert/strict');
const model = require('../../surfaces/tether/web/model.js');
const frame = require('../../fixtures/hub-frame.json');
test('notification commands are for this instance and are not repeated', () => {
  const seen = new Set();
  const frame = { notifications: [
    { id: 'n1', target: 'tether', title: 'Approval needed', body: 'return' },
    { id: 'n2', target: 'macos', title: 'Other', body: '' },
    { id: '', target: 'tether', title: 'nope', body: '' },
    { id: 'n1', target: 'tether', title: 'Approval needed', body: 'return' },
  ]};
  assert.deepEqual(model.takeNotifications(frame, 'tether', seen), [{ id: 'n1', title: 'Approval needed', body: 'return' }]);
  assert.deepEqual(model.takeNotifications(frame, 'tether', seen), []);
  const note = model.takeNotifications({ notifications: [{ id: 'n3', target: 'tether', title: '  ', body: 'x'.repeat(300) }] }, 'tether', seen)[0];
  assert.equal(note.title, 'Nerve');
  assert.equal(note.body.length, 240);
});
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
test('rejects malformed snapshots instead of replacing valid state with an empty list', () => {
  for (const patch of [{jobs:null},{jobs:[null]},{jobs:[{id:'bad'}]},
    {jobs:[{id:'bad',display:{status:'unknown'}}]}, {revision:NaN}, {revision:-1}, {hubInstanceId:null},
    {machines:[null]}, {jobs:[frame.jobs[0],frame.jobs[0]]},
    {jobs:[{...frame.jobs[0],timeline:[null]}]}]) {
    assert.throws(()=>model.jobs({...frame,...patch}),/Invalid/);
  }
});
test('a click target keeps the terminal, workspace and prompt the hub already named', () => {
  const job = {
    extensions: { terminal: { host: 'studio', terminal: '/dev/ttys001' }, lastPrompt: 'ship it' },
    context: { workspace: '/tmp/nerve' }, location: { openURL: 'file:///tmp/nerve' }
  };
  assert.deepEqual(model.jump(job), {
    target: { host: 'studio', terminal: '/dev/ttys001' },
    directory: '/tmp/nerve', url: 'file:///tmp/nerve', prompt: 'ship it'
  });
  assert.equal(model.jump({}).target, null);
  assert.equal(model.jump({}).directory, '');
});
test('tab marks translate hub-selected states without classifying or deduplicating jobs', () => {
  const target = {host:'studio',terminal:'/dev/ttys001'};
  assert.deepEqual(model.tabIndicators({tabIndicators:[{target,status:'attention',label:'Needs you'}]}),
    [{target,color:model.colors[1],label:'Needs you'}]);
  assert.deepEqual(model.tabIndicators({}),[]);
  assert.deepEqual(model.tabIndicators({tabIndicators:[{target,status:'unknown'}]}),[]);
});
