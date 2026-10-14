import importlib.machinery, importlib.util, tempfile, io, json
from contextlib import redirect_stdout
from pathlib import Path
from gi.repository import Gio, GLib
loader=importlib.machinery.SourceFileLoader('bridge',str(Path(__file__).resolve().parents[1] / 'bin/focus-media'))
spec=importlib.util.spec_from_loader(loader.name,loader);m=importlib.util.module_from_spec(spec);loader.exec_module(m)
class Fake:
 def __init__(self,p): self.RUNTIME=Path(p);self.events=[]
 def sync_other_media(self,a): self.events.append(a)
 def status(self): return {'phase':'Playing','title':'Test','artist':'Artist','track_id':'test','volume':25}
with tempfile.TemporaryDirectory() as d:
 fake=Fake(d);bridge=m.Bridge(fake)
 stream=io.StringIO()
 with redirect_stdout(stream):bridge.tick()
 assert json.loads(stream.getvalue())['phase']=='Playing'
 stream=io.StringIO()
 with redirect_stdout(stream):bridge.tick()
 assert not stream.getvalue(), 'Unchanged status should not generate repeated UI work'
 bridge.status_sent=0
 with redirect_stdout(stream):bridge.tick()
 assert json.loads(stream.getvalue())['phase']=='Playing', 'Heartbeat must keep fallback polling asleep'
 context=GLib.MainContext.default()
 for i in range(20):
  while context.pending(): context.iteration(False)
 names=bridge.call('org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus','ListNames')[0]
 assert m.NAME in names,names
 import subprocess,os,time
 state=Path(d)/'peer-state';state.write_text('Playing')
 peer=subprocess.Popen(['/usr/bin/python3',str(Path(__file__).with_name('mpris_peer.py'))],env=dict(os.environ,PEER_STATE=str(state)))
 time.sleep(.2)
 bridge.tick();assert fake.events[-1] is True,fake.events
 assert not bridge.owner
 state.write_text('Paused');bridge.tick();assert fake.events[-1] is False;assert bridge.owner
 peer.terminate();peer.wait()
 import threading
 loop=GLib.MainLoop();thread=threading.Thread(target=loop.run, daemon=True);thread.start()
 other=Gio.DBusConnection.new_for_address_sync(os.environ['DBUS_SESSION_BUS_ADDRESS'],Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT|Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,None,None)
 # Read public properties through a real second D-Bus client.
 import time;time.sleep(.05)
 result=other.call_sync(m.NAME,m.PATH,m.PROPS,'GetAll',GLib.Variant('(s)',(m.PLAYER,)),None,Gio.DBusCallFlags.NONE,1000,None).unpack()[0]
 assert result['Metadata']['xesam:title']=='Test',result
 actions=[];bridge.run=lambda *args:actions.append(args)
 for method in ['PlayPause','Next','Previous']:
  other.call_sync(m.NAME,m.PATH,m.PLAYER,method,None,None,Gio.DBusCallFlags.NONE,1000,None)
 assert actions==[('toggle',),('skip','keep-pause'),('previous','keep-pause')],actions
 loop.quit();thread.join();print('Real session D-Bus: properties, handoff, restore, and three media methods passed')
