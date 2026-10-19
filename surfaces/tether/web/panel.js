'use strict';
(() => {
  const model = globalThis.NerveModel;
  const bridge = globalThis.tether;
  const background = bridge?.background;
  const endpoint = 'http://127.0.0.1:17890/v1/stream?surface=tether';
  let instance, revision = -1, stream;
  let summary = {}, current = [], machines = [], filter = 'all', phase = 'connecting';
  let indicators = [];
  let disposed = false, pending = false, retry = null, delay = 1000;
  const rows = new Map(), headings = new Map();
  const container = document.getElementById('jobs');
  const empty = document.getElementById('empty');
  const connection = document.getElementById('connection');
  const retryButton = document.getElementById('retry');
  const searchToggle = document.getElementById('search-toggle');
  const searchBox = document.getElementById('search-box');
  const search = document.getElementById('search');
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
  function renderRow(job, offline) {
    let row = rows.get(job.id);
    if (!row) {
      row = element('details', 'row');
      row.dataset.jobId = job.id;
      const toggle = element('summary', '');
      toggle.append(circle(job.status), element('span', 'title'));
      row.append(toggle, element('div', 'row-content'));
      rows.set(job.id, row);
    }
    row.classList.toggle('offline', offline);
    const toggle = row.firstElementChild;
    const label = [job.name || job.id, job.display.statusLabel,
      job.display.machineLabel, offline ? 'Machine offline · last snapshot' : '',
      phase !== 'live' ? phases[phase][1] : ''].filter(Boolean).join(' · ');
    toggle.title = label;
    toggle.setAttribute('aria-label', label);
    toggle.firstElementChild.dataset.status = job.status;
    toggle.lastElementChild.textContent = job.name || job.id;
    // Keep native disclosure and focused summary nodes alive across snapshots.
    const content = row.lastElementChild;
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
    return row;
  }
  function render() {
    if (background) return;
    document.body.dataset.connection = phase;
    const [status, label] = phases[phase];
    connection.dataset.status = status;
    connection.title = label;
    connection.setAttribute('aria-label', label);
    retryButton.hidden = phase === 'live' || phase === 'connecting' || phase === 'unavailable';
    container.classList.toggle('stale', phase !== 'live');
    container.setAttribute('aria-label', phase === 'live' ? 'Agent jobs' : 'Agent jobs · last valid snapshot');
    const selected = model.select(current, filter, search.value);
    const groups = new Map();
    for (const job of selected) {
      const id = job.display.machineGroup;
      if (!groups.has(id)) groups.set(id, { label: job.display.machineLabel, jobs: [] });
      groups.get(id).jobs.push(job);
    }
    if (filter === 'all' && !search.value.trim()) {
      for (const machine of machines) {
        const id = machine.machineId || machine.id;
        if (!groups.has(id)) groups.set(id, { label: machine.host || id, jobs: [] });
      }
    }
    const active = document.activeElement;
    const focusedID = active?.closest('.row')?.dataset.jobId;
    const oldIndex = [...container.querySelectorAll('summary')].indexOf(active);
    const nodes = [];
    for (const [id, group] of groups) {
      const machine = machines.find(value => value.machineId === id || value.id === id);
      const offline = machine ? machine.state !== 'connected' : group.jobs.some(job => job.display.sourceOnline === false);
      if (groups.size > 1 || offline) {
        let heading = headings.get(id);
        if (!heading) { heading = element('div', 'machine'); headings.set(id, heading); }
        heading.textContent = group.label || id;
        heading.title = machine ? [machine.host, machine.state, machine.error].filter(Boolean).join(' · ')
          : offline ? 'Machine offline · last snapshot' : group.label;
        if (offline) heading.prepend(circle('inactive'));
        nodes.push(heading);
      }
      for (const job of group.jobs) nodes.push(renderRow(job, offline));
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
    if (focusedID && !container.contains(document.activeElement)) {
      const toggles = [...container.querySelectorAll('summary')];
      const target = rows.get(focusedID)?.isConnected ? rows.get(focusedID).firstElementChild
        : toggles[Math.min(Math.max(oldIndex, 0), toggles.length - 1)];
      (target || searchToggle).focus({ preventScroll: true });
    }
    empty.hidden = selected.length > 0 || nodes.length > 0;
    const message = phase !== 'live' ? label : current.length ? 'No matching jobs' : 'No active jobs · waiting for agent events';
    empty.title = message;
    empty.setAttribute('aria-label', message);
    empty.firstElementChild.dataset.status = phase !== 'live' ? status : 'inactive';
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
      clearTimeout(retry); retry = null;
      phase = 'live'; delay = 1000; render(); report();
    } catch (error) {
      phase = error.message.includes('protocol version') ? 'upgrade' : 'invalid';
      render(); report();
    }
  };
  for (const [index, status] of ['all', ...model.order].entries()) {
    const button = element('button', 'filter');
    button.dataset.filter = status;
    button.title = index === 0 ? 'All jobs' : model.labels[index - 1];
    button.setAttribute('aria-label', button.title);
    button.setAttribute('aria-pressed', String(status === filter));
    button.append(circle(status));
    button.addEventListener('click', () => {
      filter = filter === status ? 'all' : status;
      for (const other of document.querySelectorAll('[data-filter]')) other.setAttribute('aria-pressed', String(other.dataset.filter === filter));
      render();
    });
    document.getElementById('filters').append(button);
  }
  searchToggle.addEventListener('click', () => {
    searchBox.hidden = !searchBox.hidden;
    searchToggle.setAttribute('aria-expanded', String(!searchBox.hidden));
    if (!searchBox.hidden) search.focus();
    else { search.value = ''; render(); }
  });
  search.addEventListener('input', render);
  retryButton.addEventListener('click', () => { clearTimeout(retry); retry = null; delay = 1000; connect(); });
  document.addEventListener('keydown', event => {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'f') {
      event.preventDefault(); searchBox.hidden = false;
      searchToggle.setAttribute('aria-expanded', 'true'); search.focus(); search.select(); return;
    }
    if (event.key === 'Escape') {
      if (search.value) { search.value = ''; render(); }
      else if (!searchBox.hidden) { searchBox.hidden = true; searchToggle.setAttribute('aria-expanded', 'false'); searchToggle.focus(); }
      else {
        const row = document.activeElement?.closest('details[open]');
        if (!row) return;
        row.open = false; row.firstElementChild.focus();
      }
      event.preventDefault(); event.stopPropagation(); return;
    }
    if (!['ArrowUp', 'ArrowDown', 'Home', 'End'].includes(event.key) || event.target === search
      || event.metaKey || event.ctrlKey || event.altKey || event.shiftKey) return;
    const toggles = [...container.querySelectorAll('summary')];
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
