"""Automatic interruption must preserve user intent and base volume."""
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from test_focus_music import load_player


class OtherMediaTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.player = load_player(Path(self.directory.name))
        self.player.save({'genre': 'lofi-hiphop', 'volume': 40})
        self.props = {'pause': False, 'idle-active': False, 'volume': 40}
        def ipc(command):
            if command[0] == 'set_property':
                self.props[command[1]] = command[2]
        self.mock = patch.object(self.player, 'ipc', side_effect=ipc)
        self.mock.start()
        self.addCleanup(self.mock.stop)
        prop = patch.object(self.player, 'prop', side_effect=lambda key: self.props[key])
        prop.start()
        self.addCleanup(prop.stop)

    def test_pauses_and_resumes_only_automatic_pauses(self):
        self.player.sync_other_media(True)
        self.assertTrue(self.props['pause'])
        self.assertTrue(self.player.state()['auto_paused'])
        self.player.sync_other_media(False)
        self.assertFalse(self.props['pause'])
        self.props['pause'] = True
        self.player.sync_other_media(True)
        self.player.sync_other_media(False)
        self.assertTrue(self.props['pause'])

    def test_manual_pause_during_interruption_cancels_resume(self):
        self.player.sync_other_media(True)
        saved = self.player.state()
        saved['auto_paused'] = False  # explicit playback actions clear ownership
        self.player.save(saved)
        self.player.sync_other_media(False)
        self.assertTrue(self.props['pause'])

    def test_ducking_preserves_base_volume_and_restores_new_volume(self):
        for mode, factor in (('duck-strong', .4), ('duck-gentle', .75), ('none', 1)):
            self.player.configure({'otherMedia': mode})
            self.player.sync_other_media(True)
            self.assertFalse(self.props['pause'])
            self.assertEqual(self.props['volume'], 40 * factor)
            self.assertEqual(self.player.state()['volume'], 40)
            saved = self.player.state()
            saved['volume'] = 60
            self.player.save(saved)
            self.player.sync_other_media(False)
            if factor != 1:
                self.assertEqual(self.props['volume'], 60)
            saved['volume'] = 40
            self.player.save(saved)
            self.props['volume'] = 40

    def test_changing_mode_restores_automatic_pause_and_volume(self):
        self.player.sync_other_media(True)
        self.player.configure({'otherMedia': 'duck-gentle'})
        self.player.sync_other_media(True)
        self.assertFalse(self.props['pause'])
        self.assertEqual(self.props['volume'], 30)
        self.player.configure({'otherMedia': 'none'})
        self.player.sync_other_media(True)
        self.assertEqual(self.props['volume'], 40)
