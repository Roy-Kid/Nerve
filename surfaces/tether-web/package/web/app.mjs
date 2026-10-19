import {colors,order,statusOf,visibleJobs,readFrame} from './model.mjs';
const bridge = window.chrome?.webview;
const el = id => document.getElementById(id);
const state = {session:null,context:{hostLabel:'localhost',theme:'light'},jobs:[],scoped:false,subscription:null,timer:null,generation:0,failures:0,online:false};
const pending = new Map();
function rpc(method,payload) {
  if (!bridge || !state.session) return Promise.reject(Error('Open this plugin in Tether'));
  const id = crypto.randomUUID();
  return new Promise((resolve,reject) => {
    const timer = setTimeout(() => { pending.delete(id); reject(Error('Host request timed out')); },15000);
    pending.set(id,{resolve,reject,timer});
    bridge.postMessage({api:1,session:state.session,id,method,payload});
  });
}
function text(tag,value,className) { const node=document.createElement(tag); node.textContent=typeof value==='string'?value:''; if(className)node.className=className; return node; }
function render() {
  document.documentElement.dataset.theme=state.context.theme;
  el('connection').textContent=state.online?'Live':'Offline';
  const jobs=visibleJobs(state.jobs,{query:el('query').value,host:state.context.hostLabel,scoped:state.scoped});
  el('count').textContent=String(jobs.length);
  const scope=el('scope'); scope.setAttribute('aria-pressed',String(state.scoped)); scope.title=scope.ariaLabel=state.scoped?`This host: ${state.context.hostLabel}`:'All machines';
  const expanded=new Set([...el('jobs').querySelectorAll('details[open]')].map(node=>node.dataset.id));
  el('jobs').replaceChildren(); el('ribbon').replaceChildren();
  for(const status of order) {
    const count=jobs.filter(job=>statusOf(job)===status).length; if(!count)continue;
    const band=document.createElement('span'); band.className='band'; band.style.flex=String(count);band.style.background=colors[status];band.title=`${status}: ${count}`;el('ribbon').append(band);
  }
  const groups=new Map(); for(const job of jobs) {const key=job.alias||'localhost';if(!groups.has(key))groups.set(key,[]);groups.get(key).push(job);}
  for(const [alias,group] of groups) {
    el('jobs').append(text('h2',alias));
    for(const job of group) {
      const status=statusOf(job), detail=document.createElement('details');detail.dataset.id=job.id;detail.open=expanded.has(job.id);
      const row=document.createElement('summary');row.setAttribute('aria-label',`${job.name||job.id}, ${status}`);
      const dot=text('span','','dot');dot.style.background=colors[status];row.append(dot,text('span',job.name||job.id,'name'));
      row.append(text('span',job.attention?.summary||job.current?.summary||job.current?.name||'', 'activity'));
      const label=status==='success' && job.lifecycle==='active'?'Turn complete':status;
      row.append(text('span',[label,job.extensions?.model||job.producer?.name].filter(Boolean).join(' · '),'metadata'));
      detail.append(row);
      const body=text('div','','detail');
      for(const value of [job.attention?.title,job.current?.detail,job.extensions?.lastPrompt,job.context?.workspace,job.context?.project])if(value)body.append(text('p',value));
      const timeline=Array.isArray(job.timeline)?job.timeline.slice(-5):[];
      for(const event of timeline)body.append(text('p',event.title||event.kind));
      body.append(text('code',job.id));detail.append(body);el('jobs').append(detail);
    }
  }
  el('empty').hidden=jobs.length>0; el('empty').textContent=state.online?'No jobs':'Hub offline';
}
async function reconnect() {
  clearTimeout(state.timer);const generation=++state.generation;
  const old=state.subscription;state.subscription=null;state.online=false;render();
  if(old)await rpc('network.unsubscribe',{subscription:old}).catch(()=>{});
  try {
    await rpc('service.ensure',{service:'app.nerve.hub'});
    const {subscription}=await rpc('network.subscribe',{url:'http://127.0.0.1:17890/v1/stream?surface=tether-web'});
    if(generation!==state.generation) {await rpc('network.unsubscribe',{subscription});return;}
    state.subscription=subscription;
  } catch(error) {if(generation===state.generation) offline(error.message);}
}
function offline(message) {
  state.online=false;el('problem').textContent=message;render();
  clearTimeout(state.timer);state.timer=setTimeout(reconnect,Math.min(1000*2**Math.min(state.failures++,5),30000));
}
bridge?.addEventListener('message',({data})=> {
  if(data.api!==1)return;
  if(data.type==='ready') {state.session=data.session;state.context=data.context;reconnect();}
  else if(data.type==='context') {state.context=data.context;render();}
  else if(data.type==='command' && data.id==='reconnect') reconnect();
  else if(data.type==='response') {
    const request=pending.get(data.id);if(!request)return;pending.delete(data.id);clearTimeout(request.timer);
    data.error?request.reject(Error(data.error)):request.resolve(data.result);
  } else if(data.type==='event' && data.subscription===state.subscription) {
    if(data.error) {offline(data.error);return;}
    try {state.jobs=readFrame(data.text);state.online=true;state.failures=0;el('problem').textContent='';render();}
    catch(error) {offline(error.message);}
  }
});
el('query').addEventListener('input',render);el('scope').addEventListener('click',()=>{state.scoped=!state.scoped;render();});el('reconnect').addEventListener('click',reconnect);
window.addEventListener('pagehide',()=>{clearTimeout(state.timer);if(state.subscription)rpc('network.unsubscribe',{subscription:state.subscription}).catch(()=>{});});
render();if(!bridge)el('problem').textContent='Open this plugin in Tether';
