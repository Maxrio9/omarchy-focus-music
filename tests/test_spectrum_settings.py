"""Frequency response, exact player selection, and persistent UI preferences."""
import json
import importlib.util
import os
import math
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

from test_focus_music import ROOT, load_player

sys.path.insert(0, str(ROOT / 'lib'))
from focus_spectrum import SIZE, RATE, spectrum, spectra, player_node, bar_count
import focus_preferences


class SpectrumTests(unittest.TestCase):
    def test_five_bands_follow_actual_frequency_and_ignore_silence(self):
        self.assertEqual(spectrum([0] * SIZE), [0] * 5)
        for band, frequency in enumerate((100, 300, 1000, 3000, 7000)):
            samples = [0.15 * math.sin(2 * math.pi * frequency * i / RATE) for i in range(SIZE)]
            values = spectrum(samples)
            self.assertEqual(values.index(max(values)), band)
            self.assertGreater(values[band], 0.5)

    def test_pre_volume_levels_match_for_different_mpv_volume_settings(self):
        reference = [0.15 * math.sin(2 * math.pi * 1000 * i / RATE) for i in range(SIZE)]
        loud = spectrum(reference, 100)
        quiet = spectrum([value * 0.2 ** 3 for value in reference], 20)
        for a, b in zip(loud, quiet):
            self.assertAlmostEqual(a, b, places=5)

    def test_shared_fft_matches_independent_band_results(self):
        samples = [.15 * math.sin(2 * math.pi * 1000 * i / RATE) for i in range(SIZE)]
        for count in (5, 6, 14, 30):
            shared = spectra(samples, 35, (count, 5))
            self.assertEqual(shared[0], spectrum(samples, 35, count))
            self.assertEqual(shared[1], spectrum(samples, 35, 5))

    def test_wider_bar_pitch_and_idle_count(self):
        self.assertEqual(bar_count(80), 5)
        self.assertEqual(bar_count(120), 6)
        self.assertEqual(bar_count(240), 14)
        self.assertEqual(bar_count(500), 30)
        for width in (80, 120, 240, 500):
            count = bar_count(width)
            self.assertEqual(spectrum([0] * SIZE, count=count), [0] * count)

    def test_variable_band_counts_preserve_frequency_response(self):
        samples = [0.15 * math.sin(2 * math.pi * 1000 * i / RATE) for i in range(SIZE)]
        for count in (8, 20, 48):
            values = spectrum(samples, count=count)
            self.assertEqual(len(values), count)
            self.assertGreater(max(values), 0.5)
            frequency = 40 * 250 ** ((values.index(max(values)) + 0.5) / count)
            self.assertLess(abs(frequency - 1000), 300)
            self.assertEqual(spectrum([0] * SIZE, count=count), [0] * count)

    def test_selects_only_the_players_output_not_other_apps_or_microphone(self):
        objects = [
            {'id': 1, 'type': 'PipeWire:Interface:Client', 'info': {'props': {'application.process.id': 123}}},
            {'id': 2, 'type': 'PipeWire:Interface:Node', 'info': {'props': {'client.id': 1, 'media.class': 'Audio/Source', 'object.serial': 200}}},
            {'id': 3, 'type': 'PipeWire:Interface:Node', 'info': {'props': {'client.id': 99, 'media.class': 'Stream/Output/Audio', 'object.serial': 300}}},
            {'id': 4, 'type': 'PipeWire:Interface:Node', 'info': {'props': {'client.id': 1, 'media.class': 'Stream/Output/Audio', 'object.serial': 400}}},
        ]
        self.assertEqual(player_node(objects, 123), 400)
        self.assertIsNone(player_node(objects, 987))


class PreferencesTests(unittest.TestCase):
    def test_preferences_persist_and_right_placement_selects_icon_and_disables_compact(self):
        with tempfile.TemporaryDirectory() as directory:
            player = load_player(Path(directory))
            with patch.object(player.subprocess, 'run') as commands:
                updated = player.configure({'placement': 'right', 'otherMedia': 'duck-gentle'})
            self.assertEqual(updated['barDisplay'], 'icon')
            self.assertEqual(updated['otherMedia'], 'duck-gentle')
            self.assertEqual(player.preferences(), updated)
            self.assertEqual(player.PREFERENCES.stat().st_mode & 0o777, 0o600)
            calls = [call.args[0] for call in commands.call_args_list]
            self.assertEqual(calls[0][-1], '--disable')
            self.assertEqual(calls[1][-4:], ['--section', 'right', '--index', '0'])
            before = player.PREFERENCES.read_bytes()
            for invalid in ({'barDisplay': 'fake'}, {'otherMedia': 'invalid'}, {'bannerWidth': 10000}, {'unknown': 1}):
                with self.assertRaises(ValueError):
                    player.configure(invalid)
                self.assertEqual(player.PREFERENCES.read_bytes(), before)

    def test_deprecated_visualizer_preferences_do_not_reset_other_settings(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'preferences.json'
            path.write_text(json.dumps({'popupVisualizer': False, 'volumeReactive': False,
                                        'barDisplay': 'visualizer', 'bannerWidth': 240}))
            prefs = focus_preferences.read(path)
            self.assertEqual(prefs['barDisplay'], 'visualizer')
            self.assertEqual(prefs['bannerWidth'], 240)
            self.assertNotIn('popupVisualizer', prefs)
            self.assertEqual(prefs['visualizerWidth'], 100)

    def test_infers_existing_manual_bar_position_before_preferences_exist(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            shell = root / 'shell.json'
            shell.write_text(json.dumps({'bar': {'layout': {'right': [{'id': 'maxrio9.focus-music'}]}}}))
            prefs = focus_preferences.read(root / 'missing.json', shell)
            self.assertEqual(prefs['placement'], 'right')
            self.assertEqual(prefs['barDisplay'], 'icon')


class CompactDisableTests(unittest.TestCase):
    def test_right_icon_cleanup_only_restores_recognized_focus_music_blocks(self):
        spec = importlib.util.spec_from_file_location('compact_off', ROOT / 'scripts/configure-bar.py')
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            config = root / 'config'
            plugins = config / 'omarchy/plugins'
            known = plugins / 'custom.bar'
            known.mkdir(parents=True)
            (known / 'manifest.json').write_text(json.dumps({'omarchy': {'clonedFrom': 'omarchy.bar'}, 'entryPoints': {'bar': 'Bar.qml'}}))
            (known / 'Bar.qml').write_text(module.RIGHT_COMPACT + module.HOVER_FUNCTION)
            unrelated = plugins / 'other.bar'
            unrelated.mkdir()
            (unrelated / 'manifest.json').write_text((known / 'manifest.json').read_text())
            original = 'An unrelated custom bar'
            (unrelated / 'Bar.qml').write_text(original)
            with patch.dict(os.environ, {'XDG_CONFIG_HOME': str(config), 'XDG_STATE_HOME': str(root / 'state')}):
                module.disable_compact()
            self.assertEqual((known / 'Bar.qml').read_text(), module.RIGHT_ORIGINAL + module.HOVER_FUNCTION)
            self.assertEqual((unrelated / 'Bar.qml').read_text(), original)
            self.assertTrue(list((root / 'state/focus-music/backups').rglob('*-Bar.qml')))
            with patch.dict(os.environ, {'XDG_CONFIG_HOME': str(config), 'XDG_STATE_HOME': str(root / 'state')}):
                module.restore_compact()
            self.assertEqual((known / 'Bar.qml').read_text(), module.RIGHT_COMPACT + module.HOVER_FUNCTION)
            self.assertEqual((unrelated / 'Bar.qml').read_text(), original)
