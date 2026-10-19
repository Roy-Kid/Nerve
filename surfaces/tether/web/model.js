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
  function tabIndicators(frame) {
    return (frame.tabIndicators || []).flatMap(item => {
      const index = order.indexOf(item.status);
      return index < 0 ? [] : [{target:item.target, color:colors[index], label:item.label}];
    });
  }
  const api = { order, labels, colors, classify, jobs, select, status, activity, tabIndicators };
  root.NerveModel = api;
  if (typeof module !== 'undefined') module.exports = api;
})(globalThis);
