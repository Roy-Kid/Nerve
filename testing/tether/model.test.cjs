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
test('an empty status bucket says so instead of leaving a bare dot', () => {
  assert.equal(model.listEmptyCopy('waiting', '', 4), 'Nothing waiting');
  assert.equal(model.listEmptyCopy('attention', '', 4), 'Nothing needs you');
  assert.equal(model.listEmptyCopy('running', '', 4), 'Nothing running');
  assert.equal(model.listEmptyCopy('all', '', 0), 'No active jobs');
  assert.equal(model.listEmptyCopy('all', '  codex ', 4), 'No matching jobs');
  assert.equal(model.listEmptyCopy('unknown', '', 4), 'No matching jobs');
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
test('groups jobs the same three ways as the macOS panel', () => {
  const rows = model.jobs(frame);
  assert.deepEqual(model.groupModes.map(mode => mode.id), ['machine', 'priority', 'status']);
  assert.deepEqual(model.sections(rows, 'machine').map(section => [section.title, section.jobs.map(job => job.id)]), [
    ['fixture-machine', ['demo-4', 'demo-3', 'demo-2', 'demo-1']],
  ]);
  assert.deepEqual(model.sections(rows, 'priority').map(section => [section.title, section.jobs.map(job => job.id)]), [
    ['Needs you', ['demo-4', 'demo-3']],
    ['Active', ['demo-2', 'demo-1']],
  ]);
  assert.deepEqual(model.sections(rows, 'status').map(section => section.title), ['Problem', 'Needs you', 'Running']);
  const withEmpty = model.sections(rows, 'machine', [{ machineId: 'other', host: 'laptop', state: 'connected' }], true);
  assert.deepEqual(withEmpty.map(section => section.title), ['fixture-machine', 'laptop']);
  assert.equal(withEmpty[1].jobs.length, 0);
  const offline = { ...rows[0], id: 'gone', display: { ...rows[0].display, sourceOnline: false, machineGroup: 'down', machineLabel: 'down' } };
  assert.equal(model.sections([offline], 'machine').length, 0);
  assert.equal(model.sections(rows, 'machine', [{ machineId: 'fixture-machine-id', state: 'offline' }]).length, 0);
  const hidden = { ...rows[0], id: 'gone', display: { ...rows[0].display, sourceOnline: false, status: 'running' } };
  const shown = model.tallies(rows);
  assert.equal(model.tallies([...rows, hidden]).running, shown.running);
  assert.equal(model.tallies(rows, [{ machineId: 'fixture-machine-id', state: 'offline' }]).running, 0);
  assert.equal(model.tallies(rows, [{ machineId: 'fixture-machine-id', state: 'offline' }]).attention, 0);
});
test('tab marks translate hub-selected states without classifying or deduplicating jobs', () => {
  const target = {host:'studio',terminal:'/dev/ttys001'};
  assert.deepEqual(model.tabIndicators({tabIndicators:[{target,status:'attention',label:'Needs you'}]}),
    [{target,color:model.colors[1],label:'Needs you'}]);
  assert.deepEqual(model.tabIndicators({}),[]);
  assert.deepEqual(model.tabIndicators({tabIndicators:[{target,status:'unknown'}]}),[]);
});
