"""User-owned library indexing and offline playback/normalization checks."""
import contextlib
import importlib.util
import io
import json
import math
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import wave

from test_focus_music import ROOT, load_player
from test_normalization import load_normalizer


def audio_file(path, seconds=2):
    path.parent.mkdir(parents=True, exist_ok=True)
    samples = [int(3000 * math.sin(2 * math.pi * 440 * i / 16000))
               for i in range(int(16000 * seconds))]
    with wave.open(str(path), 'wb') as audio:
        audio.setparams((1, 2, 16000, 0, 'NONE', 'not compressed'))
        audio.writeframes(struct.pack('<' + 'h' * len(samples), *samples))


class LocalMusicTests(unittest.TestCase):
    @unittest.skipUnless(shutil.which('ffprobe'), 'FFmpeg supplies the local tag/duration reader')
    def test_folder_import_reads_tags_deduplicates_symlinks_and_refreshes_genres(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            player = load_player(root)
            library = root / 'Music library ü'
            source = library / 'Piano' / 'Artist - Title.wav'
            audio_file(source)
            album = library / 'Jazz' / 'Album'
            album.mkdir(parents=True)
            tagged = album / 'Tagged.flac'
            subprocess.run(['ffmpeg', '-nostdin', '-v', 'error', '-i', str(source),
                            '-metadata', 'artist=Tagged Artist', '-metadata', 'title=Tagged Title',
                            str(tagged)], check=True)
            (library / 'Piano' / 'Duplicate.wav').symlink_to(source)
            (library / 'Piano' / 'Broken.mp3').write_text('not audio')
            initial_revision = player.CATALOG_REVISION
            output = io.StringIO()
            with patch.object(player.sys, 'argv', ['focus-music', 'local', str(library)]), contextlib.redirect_stdout(output):
                player.main()
            report = json.loads(output.getvalue())
            self.assertEqual({g['value'] for g in report['genres']}, {'local-piano', 'local-jazz'})
            self.assertEqual(sum(g['track_count'] for g in report['genres']), 2)
            self.assertEqual(report['skipped_files'], ['Piano/Broken.mp3'])
            self.assertEqual(player.LOCAL_CONFIG.stat().st_mode & 0o777, 0o600)
            fresh = load_player(root)
            self.assertNotEqual(initial_revision, fresh.CATALOG_REVISION)
            self.assertEqual(fresh.SOURCES['local-piano']['tracks'][0]['title'], 'Title')
            self.assertEqual(fresh.SOURCES['local-piano']['tracks'][0]['artist'], 'Artist')
            self.assertEqual(fresh.SOURCES['local-jazz']['tracks'][0]['artist'], 'Tagged Artist')
            self.assertEqual(fresh.SOURCES['local-jazz']['tracks'][0]['title'], 'Tagged Title')
            self.assertAlmostEqual(fresh.SOURCES['local-piano']['duration'], 2, places=2)
            self.assertIn('lofi-hiphop', fresh.SOURCES)
            self.assertTrue(all(Path(t['url']).is_file() for key, genre in fresh.SOURCES.items()
                                if key.startswith('local-') for t in genre['tracks']))

    def test_local_player_does_not_require_a_network_resolver(self):
        with tempfile.TemporaryDirectory() as directory:
            player = load_player(Path(directory))
            local = {'tracks': [{'url': str(Path(directory) / 'My file.wav')}]}
            with patch.object(player.shutil, 'which', side_effect=lambda name: None if name == 'yt-dlp' else '/test/' + name), \
                 patch.object(player, 'prop', return_value=False):
                player.ensure_player({'volume': 0}, local)
                with self.assertRaisesRegex(RuntimeError, 'Streaming requires yt-dlp'):
                    player.ensure_player({'volume': 0}, {'tracks': [{'url': 'https://www.youtube.com/watch?v=test'}]})

    @unittest.skipUnless(shutil.which('ffmpeg'), 'FFmpeg is needed for local loudness measurement')
    def test_local_normalization_never_runs_the_online_resolver(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            module = load_normalizer(root)
            source = root / 'Local ü.wav'
            audio_file(source)
            track = {'url': str(source), 'start': 0, 'end': 2, 'duration': 2, 'fingerprint': 'original'}
            with patch.object(module.subprocess, 'run', side_effect=AssertionError('Online resolver was called')):
                first = module.measurement(track)
                second = module.measurement(track)
            self.assertEqual(first, second)
            self.assertIn('loudness_lufs', first)
            self.assertNotEqual(module.cache_path(track), module.cache_path(dict(track, fingerprint='modified')))
            self.assertEqual(module.player.local_path(dict(track, url=source.as_uri())), source)

    def test_plugin_supports_both_clock_sides(self):
        with tempfile.TemporaryDirectory() as directory:
            player = load_player(Path(directory))
            for side, option in [('left', '--before'), ('right', '--after')]:
                with patch.object(player.sys, 'argv', ['focus-music', 'place', side]), \
                     patch.object(player.subprocess, 'run') as command:
                    player.main()
                self.assertEqual(command.call_args.args[0], ['omarchy', 'bar', 'move', 'maxrio9.focus-music',
                                                            '--section', 'center', option, 'omarchy.clock'])


class CompactBarTests(unittest.TestCase):
    def test_patch_retains_services_is_idempotent_and_rejects_unknown_layouts(self):
        spec = importlib.util.spec_from_file_location('configure_bar', ROOT / 'scripts/configure-bar.py')
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        source = module.RIGHT_ORIGINAL + '\n' + (module.HOVER_ORIGINAL + '\n') * 2
        source += '  function setCenterSectionHovered(hovered) {'
        patched = module.patch_bar(source)
        self.assertIn('opacity: compactHidden ? 0 : 1', patched)
        self.assertNotIn('visible: !compactHidden', patched)
        self.assertIn('slot.region !== "center"', patched)
        self.assertEqual(module.patch_bar(patched), patched)
        with self.assertRaisesRegex(RuntimeError, 'not supported'):
            module.patch_bar('unknown custom bar')
