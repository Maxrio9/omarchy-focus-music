import os
from gi.repository import Gio, GLib
bus=Gio.bus_get_sync(Gio.BusType.SESSION,None)
info=Gio.DBusNodeInfo.new_for_xml('<node><interface name="org.mpris.MediaPlayer2.Player"><property name="PlaybackStatus" type="s" access="read"/></interface></node>')
def prop(*args):
 from pathlib import Path
 return GLib.Variant('s',Path(os.environ['PEER_STATE']).read_text())
bus.register_object('/org/mpris/MediaPlayer2',info.interfaces[0],None,prop,None)
owner=Gio.bus_own_name_on_connection(bus,'org.mpris.MediaPlayer2.testvideo',Gio.BusNameOwnerFlags.NONE,None,None)
GLib.MainLoop().run()
