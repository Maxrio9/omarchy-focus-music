"""Real MPRIS properties, control methods, and independent player handoff."""
from pathlib import Path
import shutil
import subprocess
import unittest


class MprisIntegrationTests(unittest.TestCase):
    def test_private_session_handoff_and_media_keys(self):
        python = '/usr/bin/python3'
        if not shutil.which('dbus-run-session') or not Path(python).exists():
            self.skipTest('System Python and dbus-run-session required')
        if subprocess.run([python, '-c', 'from gi.repository import Gio, GLib'],
                          stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode:
            self.skipTest('Optional PyGObject media bridge dependency unavailable')
        result = subprocess.run(['dbus-run-session', '--', python,
                                 str(Path(__file__).with_name('mpris_session.py'))],
                                capture_output=True, text=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
