"""Track-wide gain checks, including real FFmpeg loudness/dynamics measurements."""
import importlib.machinery
import importlib.util
import math
import os
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import unittest
import wave
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]


def load_normalizer(directory):
    with patch.dict(os.environ, {'FOCUS_MUSIC_RUNTIME': str(directory / 'runtime'),
                                'FOCUS_MUSIC_STATE': str(directory / 'state.json'),
                                'FOCUS_MUSIC_CONFIG': str(ROOT / 'data/sources.json'),
                                'FOCUS_MUSIC_CACHE': str(directory / 'cache')}):
        loader = importlib.machinery.SourceFileLoader('focus_normalizer_test', str(ROOT / 'bin/normalize-track'))
        module = importlib.util.module_from_spec(importlib.util.spec_from_loader(loader.name, loader))
        loader.exec_module(module)
        return module


class NormalizationTests(unittest.TestCase):
    def test_peak_headroom_limits_gain_without_a_compressor(self):
        with tempfile.TemporaryDirectory() as directory:
            module = load_normalizer(Path(directory))
            self.assertEqual(module.gain_for(-18, -3), -5)
            self.assertEqual(module.gain_for(-30, -12), 7)
            self.assertEqual(module.gain_for(-40, -1), -1)
            self.assertEqual(module.gain_for(float('-inf'), -30), 0)
            self.assertEqual(module.gain_for(-80, -70), 0)

    @unittest.skipUnless(shutil.which('ffmpeg'), 'FFmpeg is needed for waveform checks')
    def test_different_recording_levels_match_while_intratrack_dynamics_survive(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            module = load_normalizer(root)
            levels = []
            for name, scale in [('quiet', .4), ('loud', 1.0)]:
                source, output = root / (name + '.wav'), root / (name + '-normalized.wav')
                samples = []
                for i in range(44100 * 8):
                    amplitude = (.02 if i < 44100 * 4 else .12) * scale
                    samples.append(int(32767 * amplitude * math.sin(2 * math.pi * 440 * i / 44100)))
                with wave.open(str(source), 'wb') as audio:
                    audio.setparams((1, 2, 44100, 0, 'NONE', 'not compressed'))
                    audio.writeframes(struct.pack('<' + 'h' * len(samples), *samples))
                measured = module.measure_source(str(source), 0, 8)
                gain = module.gain_for(measured['loudness_lufs'], measured['true_peak_dbfs'])
                subprocess.run(['ffmpeg', '-nostdin', '-hide_banner', '-loglevel', 'error',
                                '-i', str(source), '-af', f'volume={gain}dB', str(output)], check=True)
                normalized = module.measure_source(str(output), 0, 8)
                levels.append(normalized['loudness_lufs'])
                self.assertLessEqual(normalized['true_peak_dbfs'], -1.9)
                with wave.open(str(output), 'rb') as audio:
                    data = audio.readframes(audio.getnframes())
                    values = struct.unpack('<' + 'h' * (len(data) // 2), data)
                half = len(values) // 2
                rms_quiet = math.sqrt(sum(v * v for v in values[:half]) / half)
                rms_loud = math.sqrt(sum(v * v for v in values[half:]) / half)
                self.assertAlmostEqual(rms_loud / rms_quiet, 6, delta=.02)
            self.assertAlmostEqual(levels[0], levels[1], delta=.2)
            self.assertAlmostEqual(levels[0], -23, delta=.2)

    def test_cache_retains_measurements_without_streaming_urls(self):
        with tempfile.TemporaryDirectory() as directory:
            module = load_normalizer(Path(directory))
            track = {'url': 'https://www.youtube.com/watch?v=abcdefghijk', 'start': 10, 'end': 20, 'duration': 10}
            resolved = subprocess.CompletedProcess([], 0, stdout='https://stream.test/audio?signature=temporary', stderr='')
            with patch.object(module.subprocess, 'run', return_value=resolved) as resolver, \
                 patch.object(module, 'measure_source', return_value={'loudness_lufs': -24, 'true_peak_dbfs': -7}) as scanner:
                first = module.measurement(track)
                second = module.measurement(track)
                self.assertEqual(first, second)
                self.assertEqual(resolver.call_count, 1)
                self.assertEqual(scanner.call_count, 1)
                contents = next(module.CACHE.glob('*.json')).read_text()
                self.assertNotIn('https://', contents)
                self.assertNotIn('signature', contents)


if __name__ == '__main__':
    unittest.main()
