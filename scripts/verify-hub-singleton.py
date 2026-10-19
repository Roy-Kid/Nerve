#!/usr/bin/env python3
"""Isolated singleton/plugin E2E. Requires an unused fixed port; never kills an existing hub."""
import concurrent.futures
import http.server
import json
import os
from pathlib import Path
import signal
import sys
import socket
import subprocess
import tempfile
import threading
import time
import urllib.request
root=Path(__file__).resolve().parents[1]
binary=root/'target/debug/nerve-hub'
base='http://127.0.0.1:17890'
probe=socket.socket()
try: probe.bind(('127.0.0.1',17890))
except OSError: raise SystemExit('Cannot test: port 17890 is already occupied; existing service left untouched')
finally: probe.close()
def read(path):
    with urllib.request.urlopen(base+path,timeout=2) as response: return json.load(response)
def run(env): return subprocess.run([str(binary),'ensure'],env=env,capture_output=True,text=True,timeout=8)
class Other(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200);self.end_headers();self.wfile.write(b'{"service":"other","ok":true}')
    def log_message(self,*args): pass
server=http.server.HTTPServer(('127.0.0.1',17890),Other)
thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
try:
    result=run(os.environ.copy())
    assert result.returncode!=0 and 'occupied' in result.stderr,result.stderr
finally: server.shutdown();server.server_close();thread.join()
print('PASS foreign service rejected')
with tempfile.TemporaryDirectory(prefix='nerve-singleton-') as home:
    env={**os.environ,'HOME':home,'USERPROFILE':home}
    pid=None
    try:
        with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
            results=list(pool.map(lambda _:run(env),range(6)))
        assert all(r.returncode==0 for r in results),[r.stderr for r in results]
        frame=read('/v1/frame');instance=frame['hubInstanceId'];pid=int(instance.split('-')[0])
        assert read('/v1/health')['protocolVersion']==2
        assert run(env).returncode==0 and read('/v1/frame')['hubInstanceId']==instance
        print('PASS concurrent ensure reuses one authoritative instance')
        with urllib.request.urlopen(base+'/v1/stream?surface=tmux&instanceId=test&notifications=false',timeout=2) as stream:
            while not stream.readline().startswith(b'data:'): pass
        # Native adapter processes only, with no surface still attached.
        for producer,script,command,event in [
          ('codex','nerve.py',['python3','-B'],'SessionStart'),
          ('claude-code','nerve.js',['node'],'SessionStart'),
          ('grok','grok-post.js',['node'],'SessionStart')]:
            payload={'hook_event_name':event,'session_id':'e2e-'+producer,'cwd':'/tmp/nerve-e2e','agentPid':os.getpid(),'slotId':producer}
            result=subprocess.run(command+[str(root/'plugins/nerve/hooks'/script)],input=json.dumps(payload),env=env,text=True,capture_output=True,timeout=3)
            assert result.returncode==0,result.stderr
        until=time.monotonic()+2
        while time.monotonic()<until:
            frame=read('/v1/frame')
            if len(frame['jobs'])==3: break
            time.sleep(.02)
        assert len(frame['jobs'])==3,frame
        assert read('/v1/health')['watchers']==0
        assert frame['hubInstanceId']==instance
        assert {j['producer']['id'] for j in frame['jobs']}=={'codex','claude-code','grok'}
        assert read('/v1/jobs')==frame['jobs']
        assert frame['summary']['total']==3
        print('PASS all three raw adapters reach hub after last surface disconnects')
        hook=base+'/v1/hook?producer=codex'
        payload={'hook_event_name':'PermissionRequest','session_id':'e2e-codex','event_id':'duplicate','slotId':'codex','cwd':'/tmp/nerve-e2e','agentPid':os.getpid()}
        request=lambda:urllib.request.Request(hook,data=json.dumps(payload).encode(),headers={'Content-Type':'application/json'},method='POST')
        with urllib.request.urlopen(request(),timeout=2) as response: first=json.load(response)
        with urllib.request.urlopen(request(),timeout=2) as response: second=json.load(response)
        assert first['applied']>0 and second['applied']==0,(first,second)
        print('PASS duplicate raw hook event is consumed once')
        if '--web' in sys.argv:
            subprocess.run(['swift','test'],cwd=root/'testing/tether',env={**os.environ,'NERVE_PLUGIN_LIVE_HUB':'1'},check=True,timeout=120)
            print('PASS Tether native WebKit consumes the live hub stream')
    finally:
        if pid:
            os.kill(pid,signal.SIGINT)
            until=time.monotonic()+4
            while time.monotonic()<until:
                try: read('/v1/health')
                except Exception: break
                time.sleep(.05)
subprocess.run(['python3','-B',str(root/'scripts/check-plugin.py')],check=True)
