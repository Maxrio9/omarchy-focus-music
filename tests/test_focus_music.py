"""Offline checks for catalog integrity, shuffle consumption and mpv IPC."""
import importlib.machinery
import importlib.util
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import threading
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]


def load_player(directory, config=None):
    environment = {
        'FOCUS_MUSIC_RUNTIME': str(directory / 'runtime'),
        'FOCUS_MUSIC_STATE': str(directory / 'state.json'),
        'FOCUS_MUSIC_CONFIG': str(config or ROOT / 'data/sources.json'),
    }
    with patch.dict(os.environ, environment):
        loader = importlib.machinery.SourceFileLoader('focus_music_test', str(ROOT / 'bin/focus-music'))
        spec = importlib.util.spec_from_loader(loader.name, loader)
        module = importlib.util.module_from_spec(spec)
        loader.exec_module(module)
    return module


class CatalogTests(unittest.TestCase):
    def test_every_genre_has_24_hours_and_valid_segments(self):
        sources = json.loads((ROOT / 'data/sources.json').read_text())
        identities, segments, names = set(), set(), set()
        for genre, source in sources.items():
            with self.subTest(genre=genre):
                self.assertGreaterEqual(source['duration'], 86400)
                self.assertEqual(source['duration'], sum(t['duration'] for t in source['tracks']))
                for track in source['tracks']:
                    self.assertGreater(track['end'], track['start'])
                    self.assertEqual(track['duration'], track['end'] - track['start'])
                    self.assertTrue(track['artist'] and track['title'])
                    self.assertTrue(track['url'].startswith('https://www.youtube.com/watch?v='))
                    segment = (track['url'], track['start'], track['end'])
                    self.assertNotIn(track['id'], identities)
                    self.assertNotIn(segment, segments)
                    self.assertNotIn((track['artist'], track['title']), names)
                    identities.add(track['id'])
                    segments.add(segment)
                    names.add((track['artist'], track['title']))

    def test_packaged_files_contain_no_personal_absolute_paths(self):
        for relative in ('BarWidget.qml', 'bin/focus-music', 'manifest.json'):
            text = (ROOT / relative).read_text()
            self.assertNotRegex(text, r'/home/[^/]+/')
            self.assertNotIn('/usr/bin/yt-dlp', text)


class DeploymentTests(unittest.TestCase):
    def test_clean_install_from_a_path_with_spaces_and_unicode(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            install = root / 'Plugin install ü'
            (install / 'bin').mkdir(parents=True)
            (install / 'data').mkdir()
            (install / 'bin/focus-music').write_bytes((ROOT / 'bin/focus-music').read_bytes())
            (install / 'data/sources.json').write_bytes((ROOT / 'data/sources.json').read_bytes())
            home = root / 'New user home ü'
            home.mkdir()
            environment = dict(os.environ)
            for key in ('FOCUS_MUSIC_CONFIG', 'FOCUS_MUSIC_RUNTIME', 'FOCUS_MUSIC_STATE',
                        'XDG_CONFIG_HOME', 'XDG_STATE_HOME'):
                environment.pop(key, None)
            environment.update(HOME=str(home), XDG_RUNTIME_DIR=str(root / 'runtime'),
                               PYTHONDONTWRITEBYTECODE='1')
            for mode in ('unset', 'empty', 'custom'):
                with self.subTest(mode=mode):
                    env = dict(environment)
                    if mode == 'empty':
                        env.update(XDG_CONFIG_HOME='', XDG_STATE_HOME='',
                                   FOCUS_MUSIC_CONFIG='', FOCUS_MUSIC_STATE='', FOCUS_MUSIC_RUNTIME='')
                    elif mode == 'custom':
                        env.update(XDG_CONFIG_HOME=str(root / 'config ü'),
                                   XDG_STATE_HOME=str(root / 'state ü'))
                    command = [sys.executable, str(install / 'bin/focus-music')]
                    result = subprocess.run(command + ['genres'], env=env, cwd=install,
                                            capture_output=True, text=True, check=True)
                    self.assertEqual(len(json.loads(result.stdout)), 5)
                    result = subprocess.run(command + ['status'], env=env, cwd=install,
                                            capture_output=True, text=True, check=True)
                    self.assertEqual(json.loads(result.stdout)['phase'], 'Stopped')
                    subprocess.run(command + ['volume', '5'], env=env, cwd=install,
                                   capture_output=True, text=True, check=True)
                    for requested, expected in [('100', 100), ('95', 95), ('-5', 0), ('30', 30)]:
                        result = subprocess.run(command + ['volume-set', requested], env=env,
                                                cwd=install, capture_output=True, text=True, check=True)
                        self.assertEqual(json.loads(result.stdout)['volume'], expected)
                    result = subprocess.run(command + ['status'], env=env, cwd=install,
                                            capture_output=True, text=True, check=True)
                    self.assertEqual(json.loads(result.stdout)['volume'], 30)
                    state_root = Path(env.get('XDG_STATE_HOME') or home / '.local/state')
                    self.assertTrue((state_root / 'focus-music/state.json').exists())
                    self.assertFalse((install / 'focus-music/state.json').exists())


class PlayerTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.player = load_player(Path(self.directory.name))
        self.player.SOURCES = {
            genre: {'label': genre, 'catalog_version': genre + '-v1', 'duration': 160,
                    'tracks': [{'id': genre + str(i), 'url': 'https://example.test/' + genre + str(i),
                                'artist': 'Artist', 'title': 'Track ' + str(i),
                                'start': i * 10, 'end': i * 10 + 40, 'duration': 40}
                               for i in range(4)]}
            for genre in ('lofi-hiphop', 'piano')}
        self.props = {'idle-active': True, 'volume': 20, 'playlist-pos': 0, 'pause': False,
                      'path': '', 'time-pos': 0}
        self.commands = []
        self.player.ensure_player = lambda saved: None
        self.player.ipc = self.fake_ipc
        self.player.prop = lambda name: self.props[name]
        self.player.ipc_many = lambda commands: (self.fake_ipc(c) for c in commands)

    def fake_ipc(self, command):
        self.commands.append(command)
        if command[0] == 'loadfile' and command[2] == 'replace':
            self.props.update({'path': command[1], 'playlist-pos': 0, 'idle-active': False,
                               'time-pos': float(command[4]['start'])})
        elif command[0] == 'playlist-prev':
            self.props['playlist-pos'] -= 1
            saved = self.player.state()
            track = self.player.SOURCES[saved['genre']]['tracks'][saved['active_order'][self.props['playlist-pos']]]
            self.props.update(path=track['url'], **{'time-pos': track['start']})
        elif command[0] == 'set_property':
            self.props[command[1]] = command[2]
        elif command[0] == 'get_property':
            return self.props[command[1]]

    def test_selection_consumes_played_tracks_and_starts_at_track_boundary(self):
        self.player.play('lofi-hiphop')
        first = self.player.state()['active_order']
        self.assertEqual(set(first), set(range(4)))
        self.props['playlist-pos'] = 1
        self.props['path'] = self.player.SOURCES['lofi-hiphop']['tracks'][first[1]]['url']
        self.props['time-pos'] = 999  # A genre selection must discard this resume position.
        self.player.play('piano')
        self.player.play('lofi-hiphop')
        remaining = self.player.state()['active_order']
        self.assertEqual(set(remaining), set(first[2:]))
        self.assertTrue(set(first[:2]).isdisjoint(remaining))
        last_replace = [c for c in self.commands if c[0] == 'loadfile' and c[2] == 'replace'][-1]
        chosen = self.player.SOURCES['lofi-hiphop']['tracks'][remaining[0]]
        self.assertEqual(float(last_replace[4]['start']), chosen['start'])
        self.assertEqual(float(last_replace[4]['end']), chosen['end'])

    def test_previous_moves_back_and_restarts_the_first_track_at_its_boundary(self):
        self.player.play('piano')
        order = self.player.state()['active_order']
        self.props['playlist-pos'] = 1
        self.props['path'] = self.player.SOURCES['piano']['tracks'][order[1]]['url']
        with patch.object(self.player.sys, 'argv', ['focus-music', 'previous']):
            self.player.main()
        self.assertEqual(self.props['playlist-pos'], 0)
        self.assertIn(['playlist-prev', 'force'], self.commands)
        self.commands.clear()
        with patch.object(self.player.sys, 'argv', ['focus-music', 'previous']):
            self.player.main()
        expected = self.player.SOURCES['piano']['tracks'][order[0]]['start']
        self.assertIn(['seek', expected, 'absolute+exact'], self.commands)
        self.assertNotIn(['playlist-prev', 'force'], self.commands)

    def test_explicit_pause_is_idempotent_and_returns_confirmed_phase(self):
        import contextlib
        import io
        self.player.play('piano')
        for requested, expected in [('1', 'Paused'), ('1', 'Paused'), ('0', 'Playing')]:
            output = io.StringIO()
            with patch.object(self.player.sys, 'argv', ['focus-music', 'pause-set', requested]):
                with contextlib.redirect_stdout(output):
                    self.player.main()
            self.assertEqual(json.loads(output.getvalue())['phase'], expected)
            self.assertEqual(self.props['pause'], requested == '1')

    def test_status_reports_actual_track_and_artist(self):
        self.player.play('piano')
        current = self.player.state()['active_order'][0]
        status = self.player.status()
        self.assertEqual(status['title'], 'Track ' + str(current))
        self.assertEqual(status['artist'], 'Artist')
        self.assertEqual(status['phase'], 'Playing')
        self.assertEqual(status['volume'], 20)
        self.assertNotIn('⏭', status['tooltip'])


class IpcTests(unittest.TestCase):
    def test_multiple_commands_ignore_unsolicited_events(self):
        with tempfile.TemporaryDirectory() as directory:
            player = load_player(Path(directory))
            errors, received = [], []
            server = socket.socket(socket.AF_UNIX)
            server.bind(str(player.SOCKET))
            server.listen(1)
            self.addCleanup(server.close)

            def serve():
                try:
                    connection, _ = server.accept()
                    with connection, connection.makefile() as stream:
                        for result in ('Playing', 25):
                            request = json.loads(stream.readline())
                            received.append(request['command'])
                            response = [{'event': 'property-change'},
                                        {'request_id': request['request_id'], 'error': 'success', 'data': result}]
                            connection.sendall(('\n'.join(json.dumps(r) for r in response) + '\n').encode())
                except Exception as error:
                    errors.append(error)

            thread = threading.Thread(target=serve, daemon=True)
            thread.start()
            result = list(player.ipc_many([['get_property', 'phase'], ['get_property', 'volume']]))
            thread.join(5)
            self.assertFalse(thread.is_alive())
            self.assertEqual(errors, [])
            self.assertEqual(result, ['Playing', 25])
            self.assertEqual(received, [['get_property', 'phase'], ['get_property', 'volume']])


if __name__ == '__main__':
    unittest.main()
