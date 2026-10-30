'use strict';
(() => {
  const model = globalThis.NerveModel;
  const bridge = globalThis.tether;
  const background = bridge?.background;
  const endpoint = 'http://127.0.0.1:17890/v1/stream?surface=tether&instanceId=tether&notifications=true';
  const deliveredNotes = new Set();
  let instance, revision = -1, stream;
  let summary = {}, current = [], machines = [], filter = 'all', phase = 'connecting';
  let indicators = [];
  let disposed = false, pending = false, retry = null, delay = 1000;
  const rows = new Map(), headings = new Map(), blanks = new Map();
  const container = document.getElementById('jobs');
  const empty = document.getElementById('empty');
  const connection = document.getElementById('connection');
  const runningCount = document.getElementById('running-count');
  const attentionChip = document.getElementById('attention');
  const attentionCount = document.getElementById('attention-count');
  const retryButton = document.getElementById('retry');
  const note = document.getElementById('note');
  const noteLabel = document.getElementById('note-label');
  const searchBox = document.getElementById('search-box');
  const search = document.getElementById('search');
  const refreshButton = document.getElementById('refresh');
  const clearButton = document.getElementById('clear');
  const groupControl = document.querySelector('.group');
  const groupSelect = document.getElementById('group');
  let groupMode = 'machine';
  const phases = {
    connecting: ['waiting', 'Connecting to Nerve hub'],
    live: ['success', 'Connected'],
    offline: ['problem', 'Hub unavailable · last snapshot retained · retry or run nerve-hub ensure'],
    denied: ['attention', 'Network access denied · allow Nerve in Tether Settings → Extensions, then retry'],
    upgrade: ['attention', 'Upgrade Nerve hub · protocol version 2 required · retry after upgrading'],
    invalid: ['problem', 'Invalid hub snapshot · last valid snapshot retained · retry to reconnect'],
    unavailable: ['inactive', 'Open this plugin in Tether to connect to Nerve hub']
  };
  const timeFormat = new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' });
  if (background) document.body.hidden = true;
  model.order.forEach((status, index) => document.documentElement.style.setProperty(
    '--' + status, '#' + model.colors[index].toString(16).padStart(6, '0')));

  function element(tag, className, text = '') {
    const node = document.createElement(tag);
    node.className = className;
    node.textContent = text;
    return node;
  }
  function circle(status) {
    const dot = element('span', 'dot');
    dot.dataset.status = status;
    dot.setAttribute('aria-hidden', 'true');
    return dot;
  }
  function timestamp(value) {
    if (!value) return null;
    const date = new Date(value);
    if (!Number.isFinite(date.getTime())) return null;
    const node = element('time', '', timeFormat.format(date));
    node.dateTime = date.toISOString();
    node.title = date.toLocaleString();
    return node;
  }
  // Same clock the macOS row uses: endedAt, else startedAt, else updatedAt.
  // Abbreviated like the panel ("41 min ago"), not a clock time.
  function rowInstant(job) {
    if (job.lifecycle === 'ended' && job.endedAt) return job.endedAt;
    return job.startedAt || job.updatedAt || '';
  }
  function relativePhrase(value) {
    const date = new Date(value);
    const ms = date.getTime();
    if (!Number.isFinite(ms)) return '';
    const seconds = Math.round((Date.now() - ms) / 1000);
    const abs = Math.abs(seconds);
    const unit = abs < 45 ? [abs, 'sec']
      : abs < 3600 ? [Math.round(abs / 60), 'min']
      : abs < 86400 ? [Math.round(abs / 3600), 'hr']
      : abs < 86400 * 7 ? [Math.round(abs / 86400), 'day']
      : [Math.round(abs / (86400 * 7)), 'wk'];
    return seconds < 0 ? `in ${unit[0]} ${unit[1]}` : `${unit[0]} ${unit[1]} ago`;
  }
  function chevron() {
    const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    svg.setAttribute('viewBox', '0 0 16 16');
    svg.setAttribute('aria-hidden', 'true');
    const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
    path.setAttribute('d', 'M6 3.5 11 8 6 12.5');
    svg.append(path);
    return svg;
  }
  function openJob(job) {
    if (!bridge) return;
    const place = model.jump(job);
    const payload = {};
    if (place.target) payload.target = place.target;
    if (place.directory) payload.directory = place.directory;
    if (place.url) payload.url = place.url;
    if (!payload.target && !payload.directory && !payload.url) return;
    bridge.call('host.focus', payload).catch(() => {});
  }
  function renderRow(job, offline) {
    let row = rows.get(job.id);
    if (!row) {
      row = element('div', 'row');
      row.dataset.jobId = job.id;
      const jump = element('button', 'jump');
      jump.type = 'button';
      const when = element('time', 'when');
      jump.append(circle('inactive'), element('span', 'name'), element('span', 'activity'), when, element('span', 'state'));
      jump.addEventListener('click', () => { if (row.job) openJob(row.job); });
      const more = element('details', 'more');
      const disclosure = element('summary', 'chevron');
      disclosure.setAttribute('aria-label', 'Details');
      disclosure.append(chevron());
      more.append(disclosure, element('div', 'row-content'));
      row.append(jump, more);
      rows.set(job.id, row);
    }
    row.job = job;
    row.classList.toggle('offline', offline);
    const jump = row.firstElementChild;
    const label = [job.name || job.id, job.display.statusLabel,
      job.display.machineLabel, offline ? 'Machine offline · last snapshot' : '',
      phase !== 'live' ? phases[phase][1] : '', 'Open session'].filter(Boolean).join(' · ');
    jump.title = label;
    jump.setAttribute('aria-label', label);
    jump.dataset.status = job.status;
    jump.querySelector('.dot').dataset.status = job.status;
    jump.querySelector('.name').textContent = job.name || job.id;
    jump.querySelector('.activity').textContent = model.activity(job);
    const when = jump.querySelector('.when');
    const instant = rowInstant(job);
    const parsed = instant ? new Date(instant) : null;
    if (parsed && Number.isFinite(parsed.getTime())) {
      when.dateTime = parsed.toISOString();
      when.title = parsed.toLocaleString();
      when.textContent = relativePhrase(instant);
    } else {
      when.removeAttribute('datetime');
      when.title = '';
      when.textContent = '';
    }
    jump.querySelector('.state').textContent = job.display.statusLabel || '';
    // Keep the disclosure and a focused jump alive across snapshots.
    const content = row.lastElementChild.lastElementChild;
    const detailKey = JSON.stringify([model.activity(job), job.producer, job.location, job.updatedAt, job.timeline]);
    if (row.detailKey !== detailKey) {
      row.detailKey = detailKey;
      const nodes = [];
      if (model.activity(job)) nodes.push(element('p', 'activity', model.activity(job)));
      const context = job.producer?.name || job.producer?.id;
      if (context) nodes.push(element('p', 'context', context));
      for (const value of new Set([job.location?.focusHint, job.location?.logPath, job.location?.openURL].filter(Boolean))) {
        nodes.push(element('p', 'location', value));
      }
      if (job.updatedAt) {
        const time = timestamp(job.updatedAt);
        if (time) { time.title = 'Updated · ' + time.title; nodes.push(time); }
      }
      if (Array.isArray(job.timeline) && job.timeline.length) {
        const timeline = element('ol', 'timeline');
        timeline.setAttribute('aria-label', 'Task timeline');
        for (const entry of job.timeline) {
          const item = element('li', '');
          const time = timestamp(entry.timestamp);
          if (time) item.append(time);
          item.append(element('span', '', entry.title || entry.kind || ''));
          timeline.append(item);
        }
        nodes.push(timeline);
      }
      content.replaceChildren(...nodes);
    }
    row.lastElementChild.hidden = !(Array.isArray(job.timeline) && job.timeline.length);
    return row;
  }
  function render() {
    if (background) return;
    document.body.dataset.connection = phase;
    const [status, label] = phases[phase];
    connection.dataset.status = phase === 'live' ? 'running' : status;
    connection.title = label;
    connection.setAttribute('aria-label', label);
    const tallies = model.tallies(current, machines);
    const running = tallies.running;
    const attention = tallies.attention;
    runningCount.textContent = phase === 'live' ? String(running) : '—';
    attentionChip.hidden = !(phase === 'live' && attention > 0);
    attentionCount.textContent = String(attention);
    attentionChip.title = attention + ' need attention';
    const quiet = phase === 'live' || phase === 'connecting' || phase === 'unavailable';
    note.hidden = quiet;
    noteLabel.textContent = quiet ? '' : label;
    clearButton.disabled = current.length === 0;
    container.classList.toggle('stale', phase !== 'live');
    container.setAttribute('aria-label', phase === 'live' ? 'Agent jobs' : 'Agent jobs · last valid snapshot');
    const selected = model.select(current, filter, search.value);
    const grouped = model.sections(
      selected, groupMode, machines,
      groupMode === 'machine' && filter === 'all' && !search.value.trim()
    );
    const groups = new Map(grouped.map(section => [section.id, section]));
    const active = document.activeElement;
    const focusedID = active?.closest('.row')?.dataset.jobId;
    const oldIndex = [...container.querySelectorAll('.jump')].indexOf(active);
    const nodes = [];
    for (const section of grouped) {
      const id = section.id;
      let heading = headings.get(id);
      if (!heading) { heading = element('div', 'machine'); headings.set(id, heading); }
      const machine = machines.find(value => value.machineId === id || value.id === id);
      heading.textContent = section.title || id;
      heading.title = machine ? [machine.host, machine.state, machine.error].filter(Boolean).join(' · ') : section.title;
      nodes.push(heading);
      if (section.jobs.length === 0) {
        let blank = blanks.get(id);
        if (!blank) { blank = element('p', 'group-empty', 'No jobs on this machine'); blanks.set(id, blank); }
        nodes.push(blank);
      }
      for (const job of section.jobs) nodes.push(renderRow(job, false));
    }
    // Reconcile only changed positions so SSE refreshes do not steal focus.
    let cursor = container.firstElementChild;
    for (const node of nodes) {
      if (node === cursor) cursor = cursor.nextElementSibling;
      else container.insertBefore(node, cursor);
    }
    while (cursor) { const next = cursor.nextElementSibling; cursor.remove(); cursor = next; }
    for (const id of rows.keys()) if (!current.some(job => job.id === id)) rows.delete(id);
    for (const id of headings.keys()) if (!groups.has(id)) headings.delete(id);
    for (const id of blanks.keys()) if (!groups.has(id) || groups.get(id).jobs.length) blanks.delete(id);
    if (focusedID && !container.contains(document.activeElement)) {
      const toggles = [...container.querySelectorAll('.jump')];
      const target = rows.get(focusedID)?.isConnected ? rows.get(focusedID).firstElementChild
        : toggles[Math.min(Math.max(oldIndex, 0), toggles.length - 1)];
      if (target) target.focus({ preventScroll: true });
    }
    empty.hidden = nodes.length > 0;
    const message = phase !== 'live' ? label : model.listEmptyCopy(filter, search.value, current.length);
    empty.title = message;
    empty.querySelector('p').textContent = nodes.length ? '' : message;
    empty.querySelector('.dot').dataset.status = phase !== 'live' ? status : 'inactive';
  }
  function report() {
    if (background) {
      bridge.call('host.status', model.status(current, phase === 'live', summary)).catch(() => {});
      bridge.call('host.tabIndicators', phase === 'live' ? indicators : []).catch(() => {});
    }
  }
  function reconnect() {
    if (disposed || retry) return;
    phase = 'offline'; render(); report();
    retry = setTimeout(() => { retry = null; connect(); }, delay);
    delay = Math.min(delay * 2, 15000);
  }
  async function connect() {
    if (disposed || pending || !bridge) return;
    pending = true;
    phase = 'connecting'; render(); report();
    try {
      if (stream) {
        const previous = stream; stream = null;
        await bridge.call('network.closeStream', { stream: previous }).catch(() => {});
      }
      if (disposed) return;
      const opened = await bridge.call('network.stream', { url: endpoint });
      if (disposed) { await bridge.call('network.closeStream', { stream: opened.stream }); return; }
      stream = opened.stream;
    } catch (error) {
      if (disposed) return;
      if (['permissionDenied', 'consentDenied', 'noSession'].includes(error.message)) {
        phase = 'denied'; render(); report();
      } else reconnect();
    } finally { pending = false; }
  }
  if (bridge) bridge.onEvent = event => {
    if (disposed || (stream && event.stream && event.stream !== stream)) return;
    if (event.type !== 'data') { reconnect(); return; }
    try {
      const frame = JSON.parse(event.payload.data);
      const jobs = model.jobs(frame);
      const nextIndicators = model.tabIndicators(frame);
      if (instance === frame.hubInstanceId && frame.revision < revision) return;
      instance = frame.hubInstanceId; revision = frame.revision;
      current = jobs; summary = frame.summary || {}; machines = Array.isArray(frame.machines) ? frame.machines : [];
      indicators = nextIndicators;
      for (const note of model.takeNotifications(frame, 'tether', deliveredNotes)) {
        bridge.call('host.notify', { title: note.title, body: note.body }).catch(() => {});
      }
      clearTimeout(retry); retry = null;
      phase = 'live'; delay = 1000; render(); report();
    } catch (error) {
      phase = error.message.includes('protocol version') ? 'upgrade' : 'invalid';
      render(); report();
    }
  };
  function postHub(path) {
    const url = 'http://127.0.0.1:17890' + path;
    // The page cannot fetch. Tether blocks every http load; the host posts
    // only to a URL the manifest already granted.
    if (bridge) return bridge.call('network.post', { url });
    return fetch(url, { method: 'POST', mode: 'no-cors' });
  }
  function applyGroupLabel() {
    const title = model.groupModes.find(mode => mode.id === groupMode)?.title || 'Machine';
    const label = 'Group by ' + title;
    groupSelect.setAttribute('aria-label', label);
    groupSelect.title = label;
    groupControl.title = label;
  }
  refreshButton.addEventListener('click', () => {
    postHub('/v1/refresh').catch(() => { refreshButton.title = 'Could not reach Nerve hub'; });
  });
  clearButton.addEventListener('click', () => {
    if (clearButton.disabled) return;
    postHub('/v1/clear').catch(() => { note.hidden = false; noteLabel.textContent = 'Could not reach Nerve hub'; });
  });
  groupSelect.addEventListener('change', () => {
    groupMode = model.groupModes.some(mode => mode.id === groupSelect.value) ? groupSelect.value : 'machine';
    applyGroupLabel();
    render();
  });
  search.addEventListener('input', render);
  retryButton.addEventListener('click', () => { clearTimeout(retry); retry = null; delay = 1000; connect(); });
  document.addEventListener('keydown', event => {
    if (event.target === groupSelect) return;
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'f') {
      event.preventDefault(); searchBox.hidden = false; search.focus(); search.select(); return;
    }
    if (event.key === 'Escape') {
      if (search.value) { search.value = ''; render(); }
      else if (!searchBox.hidden) { searchBox.hidden = true; search.blur(); }
      else {
        const row = document.activeElement?.closest('details[open]');
        if (!row) return;
        row.open = false; row.parentElement?.querySelector('.jump')?.focus();
      }
      event.preventDefault(); event.stopPropagation(); return;
    }
    if (!['ArrowUp', 'ArrowDown', 'Home', 'End'].includes(event.key) || event.target === search
      || event.metaKey || event.ctrlKey || event.altKey || event.shiftKey) return;
    const toggles = [...container.querySelectorAll('.jump')];
    if (!toggles.length) return;
    const index = toggles.indexOf(document.activeElement);
    const next = event.key === 'Home' ? 0 : event.key === 'End' ? toggles.length - 1
      : index < 0 ? (event.key === 'ArrowUp' ? toggles.length - 1 : 0)
      : Math.min(toggles.length - 1, Math.max(0, index + (event.key === 'ArrowDown' ? 1 : -1)));
    event.preventDefault(); toggles[next].focus();
  });
  window.addEventListener('pagehide', () => {
    disposed = true; clearTimeout(retry);
    if (bridge) {
      bridge.onEvent = null;
      if (stream) bridge.call('network.closeStream', { stream }).catch(() => {});
    }
  });
  if (!bridge) { phase = 'unavailable'; render(); }
  else { render(); connect(); }
})();
