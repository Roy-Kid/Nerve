/* Structured display contract, mirrored from nerve-surface-core/src/status.rs. */
(function (root) {
  const order = ['problem', 'attention', 'waiting', 'running', 'monitor', 'success', 'inactive'];
  const labels = ['Problem', 'Needs you', 'Waiting', 'Running', 'Monitor', 'Completed', 'Ready / idle'];
  // The product palette: problem, attention, waiting, running, monitor, success, idle.
  const colors = [0xFF3B30, 0xFF9F0A, 0x8E8E93, 0x0A84FF, 0xBF5AF2, 0x30D158, 0x8E8E93];
  function classify(job) { return job.display?.status || 'inactive'; }
  function jobs(frame) {
    if (frame?.protocolVersion !== 2) throw new Error('Upgrade Nerve hub: protocol version 2 required');
    if (!Array.isArray(frame.jobs) || typeof frame.hubInstanceId !== 'string'
      || !Number.isSafeInteger(frame.revision) || frame.revision < 0
      || frame.jobs.some(job => !job || typeof job.id !== 'string' || !job.display
        || !order.includes(job.display.status)
        || (Array.isArray(job.timeline) && job.timeline.some(entry => !entry || typeof entry !== 'object')))
      || (frame.machines !== undefined && (!Array.isArray(frame.machines)
        || frame.machines.some(machine => !machine || typeof machine !== 'object')))
      || new Set(frame.jobs.map(job => job.id)).size !== frame.jobs.length) throw new Error('Invalid hub snapshot');
    return frame.jobs.map(job => ({...job, status: classify(job)}));
  }
  function select(jobs, filter = 'all', query = '') {
    const needle = query.trim().toLocaleLowerCase();
    return jobs.filter(job => (filter === 'all' || job.status === filter)
      && (!needle || [job.name, job.display.machineLabel, job.producer?.name,
        job.producer?.id, job.location?.openURL, job.location?.logPath, job.location?.focusHint,
        activity(job)].some(value => typeof value === 'string' && value.toLocaleLowerCase().includes(needle))));
  }
  function status(jobs, connected, summary = {}) {
    const counts = order.map(name => summary.counts?.[name] || 0);
    return {
      label: connected ? `Nerve · ${summary.total || 0} jobs · ${counts[1]} need you · ${counts[3]} running` : 'Nerve · Disconnected',
      segments: connected ? counts.flatMap((weight, i) => weight ? [{ color: colors[i], weight }] : []) : []
    };
  }
  function activity(job) { return job.display?.activity || ''; }
  // What the list says when a search or the whole queue is empty.
  const groupModes = [
    { id: 'machine', title: 'Machine' },
    { id: 'priority', title: 'Priority' },
    { id: 'status', title: 'Status' },
  ];
  const prioritySections = [
    ['attention', 'Needs you'],
    ['active', 'Active'],
    ['recent', 'Turn complete'],
  ];
  // Same three buckets as the macOS status panel. Machine keeps first-seen
  // order; priority and status follow the hub's section and status ranks.
  function sections(jobs, mode = 'machine', machines = [], includeEmpty = false) {
    const roster = Array.isArray(machines) ? machines : [];
    const machineFor = id => roster.find(value => value && (value.machineId === id || value.id === id));
    const visible = jobs.filter(job => {
      if (job.display?.sourceOnline === false) return false;
      const machine = machineFor(job.display?.machineGroup);
      return !(machine && machine.state !== 'connected');
    });
    if (mode === 'priority') {
      return prioritySections.flatMap(([id, title]) => {
        const rows = visible.filter(job => job.display?.section === id);
        return rows.length ? [{ id, title, jobs: rows }] : [];
      });
    }
    if (mode === 'status') {
      return order.flatMap((status, index) => {
        const rows = visible.filter(job => (job.display?.status || job.status) === status);
        return rows.length ? [{ id: 'status:' + status, title: labels[index], jobs: rows }] : [];
      });
    }
    const groups = new Map();
    for (const job of visible) {
      const id = job.display?.machineGroup || 'unknown';
      if (!groups.has(id)) groups.set(id, { id, title: job.display?.machineLabel || 'unknown', jobs: [] });
      groups.get(id).jobs.push(job);
    }
    if (includeEmpty) {
      for (const machine of roster) {
        if (!machine || machine.state !== 'connected') continue;
        const id = machine.machineId || machine.id;
        if (id && !groups.has(id)) groups.set(id, { id, title: machine.host || id, jobs: [] });
      }
    }
    return [...groups.values()];
  }
  // Header chips count the same rows the list can show. Hub summary counts
  // also include an offline machine's last snapshot, which the list omits.
  function tallies(jobs, machines = []) {
    const rows = sections(jobs, 'machine', machines, false).flatMap(section => section.jobs);
    const count = status => rows.filter(job => (job.display?.status || job.status) === status).length;
    return { running: count('running'), attention: count('attention') };
  }
  function listEmptyCopy(filter, query, total) {
    if (String(query || '').trim()) return 'No matching jobs';
    const phrases = {
      problem: 'No problem jobs',
      attention: 'Nothing needs you',
      waiting: 'Nothing waiting',
      running: 'Nothing running',
      monitor: 'Nothing to monitor',
      success: 'Nothing completed',
      inactive: 'Nothing idle',
    };
    if (filter && filter !== 'all') return phrases[filter] || 'No matching jobs';
    return total > 0 ? 'No matching jobs' : 'No active jobs';
  }
  // Where a click should land: the terminal the hub named, else the workspace.
  function jump(job) {
    const terminal = job?.extensions?.terminal;
    const host = terminal?.host;
    const device = terminal?.terminal;
    const target = typeof host === 'string' && typeof device === 'string' ? { host, terminal: device } : null;
    const directory = typeof job?.context?.workspace === 'string' ? job.context.workspace : '';
    const url = typeof job?.location?.openURL === 'string' ? job.location.openURL : '';
    const prompt = typeof job?.extensions?.lastPrompt === 'string' ? job.extensions.lastPrompt : '';
    return { target, directory, url, prompt };
  }
  function tabIndicators(frame) {
    return (frame.tabIndicators || []).flatMap(item => {
      const index = order.indexOf(item.status);
      return index < 0 ? [] : [{target:item.target, color:colors[index], label:item.label}];
    });
  }
  // Hub commands addressed to this instance. `seen` drops a repeat of the same id.
  function takeNotifications(frame, instanceId, seen) {
    if (!seen || typeof seen.has !== 'function' || typeof seen.add !== 'function') return [];
    const events = Array.isArray(frame?.notifications) ? frame.notifications : [];
    const fresh = [];
    for (const event of events) {
      if (!event || event.target !== instanceId || typeof event.id !== 'string' || !event.id || seen.has(event.id)) continue;
      if (seen.size > 200) seen.delete(seen.values().next().value);
      seen.add(event.id);
      const title = typeof event.title === 'string' && event.title.trim() ? event.title.trim().slice(0, 120) : 'Nerve';
      const body = typeof event.body === 'string' ? event.body.trim().slice(0, 240) : '';
      fresh.push({ id: event.id, title, body });
    }
    return fresh;
  }
  const api = { order, labels, colors, groupModes, classify, jobs, select, sections, tallies, status, activity, listEmptyCopy, jump, tabIndicators, takeNotifications };
  root.NerveModel = api;
  if (typeof module !== 'undefined') module.exports = api;
})(globalThis);
