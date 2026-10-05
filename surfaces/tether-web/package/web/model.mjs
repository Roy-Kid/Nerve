const busy = new Set(['subagent','tool','thinking','info']);
const ask = new Set(['input','approval','auth','permission','decision','elicitation','review']);
const waits = new Set(['resource','dependency','queue','system','lock','throttle','rate','capacity','failure']);
const rank = {none:0,informational:1,suggested:2,required:3,urgent:4};
export const order = ['problem','attention','waiting','running','monitor','success','inactive'];
export const colors = {problem:'#FF3B30',attention:'#FF9F0A',waiting:'#8E8E93',running:'#0A84FF',monitor:'#BF5AF2',success:'#30D158',inactive:'#8E8E93'};
const word = value => typeof value === 'string' ? value.trim().toLowerCase() : '';
export function statusOf(job) {
  const lifecycle = job.lifecycle ?? 'active', kind = word(job.current?.type);
  if (job.outcome === 'failure' || job.health === 'unresponsive') return 'problem';
  if (lifecycle === 'ended') return ['success','partial'].includes(job.outcome) ? 'success' : 'inactive';
  const level = rank[job.attention?.level] ?? 0, reason = word(job.attention?.reason);
  if (!(lifecycle === 'active' && busy.has(kind)) && level >= 1) {
    if (waits.has(reason)) return 'waiting';
    if (ask.has(reason) || level >= 2) return 'attention';
  }
  if (['suspended','unknown'].includes(lifecycle)) return 'inactive';
  if (['pending','created'].includes(lifecycle)) return 'waiting';
  if (job.health === 'degraded') return 'problem';
  if (lifecycle === 'active' && job.outcome === 'partial') return 'monitor';
  if (busy.has(kind) && lifecycle === 'active') return 'running';
  if (['monitor','completed','waiting'].includes(kind)) return {monitor:'monitor',completed:'success',waiting:'waiting'}[kind];
  if (['idle','starting','booting'].includes(kind)) return 'inactive';
  return lifecycle === 'active' ? 'running' : 'inactive';
}
export function visibleJobs(jobs, {query='',host='localhost',scoped=false}={}) {
  const search = word(query);
  return jobs.filter(job => !scoped || word(job.alias) === word(host)).filter(job =>
    [job.name,job.alias,job.current?.summary,job.context?.workspace,job.context?.project].some(text => word(text).includes(search)))
    .sort((a,b) => order.indexOf(statusOf(a))-order.indexOf(statusOf(b)) || String(a.alias ?? '').localeCompare(String(b.alias ?? '')) || String(a.name ?? a.id).localeCompare(String(b.name ?? b.id)));
}
export function readFrame(text) {
  const frame = JSON.parse(text);
  if (!frame || !Array.isArray(frame.jobs) || frame.jobs.some(job => !job || typeof job.id !== 'string')) throw Error('Invalid hub frame');
  // Full frames are authoritative. Departed is deliberately not a job list.
  return frame.jobs;
}
