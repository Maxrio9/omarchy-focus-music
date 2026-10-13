"""Index user-supplied audio; never download or copy the music files."""
import hashlib
import json
import math
from pathlib import Path
import re
import shutil
import subprocess
from urllib.parse import unquote, urlsplit


AUDIO_EXTENSIONS = {'.mp3', '.m4a', '.flac', '.ogg', '.opus', '.wav', '.aac',
                    '.aif', '.aiff', '.alac', '.wma', '.mka', '.mp4', '.webm', '.ape', '.wv'}


def local_path(track):
    url = track['url']
    parsed = urlsplit(url)
    if parsed.scheme == 'file' and parsed.netloc in ('', 'localhost'):
        return Path(unquote(parsed.path))
    path = Path(url).expanduser()
    return path if not parsed.scheme and path.is_absolute() else None


def index_library(folder):
    root = Path(folder).expanduser().resolve()
    if not root.is_dir():
        raise ValueError('Music folder does not exist')
    if not shutil.which('ffprobe'):
        raise RuntimeError('Indexing local music requires ffprobe (included with FFmpeg)')
    sources, skipped, seen = {}, [], set()
    for genre in sorted(root.iterdir(), key=lambda path: path.name.casefold()):
        if not genre.is_dir():
            continue
        tracks = []
        for path in sorted(genre.rglob('*')):
            if not path.is_file() or path.suffix.lower() not in AUDIO_EXTENSIONS:
                continue
            resolved = path.resolve()
            if resolved in seen:
                continue
            seen.add(resolved)
            try:
                result = subprocess.run(
                    ['ffprobe', '-v', 'error', '-select_streams', 'a:0',
                     '-show_entries', 'format=duration:format_tags=artist,title,album_artist:stream=duration:stream_tags=artist,title,album_artist',
                     '-of', 'json', str(resolved)],
                    capture_output=True, text=True, timeout=30, check=True)
                metadata = json.loads(result.stdout)
                streams = metadata.get('streams', [])
                if not streams:
                    raise ValueError('No audio stream')
                info = metadata.get('format', {})
                duration = float(info.get('duration') or streams[0].get('duration') or 0)
                if not math.isfinite(duration) or duration <= 0:
                    raise ValueError('No finite duration')
                tags = {k.lower(): v for k, v in streams[0].get('tags', {}).items()}
                tags.update({k.lower(): v for k, v in info.get('tags', {}).items()})
                artist = tags.get('artist') or tags.get('album_artist') or ''
                title = tags.get('title') or path.stem
                if not artist and not tags.get('title'):
                    parts = re.split(r' (?:—|-) ', path.stem, maxsplit=1)
                    if len(parts) == 2:
                        artist, title = parts
                stat = resolved.stat()
                identity = hashlib.sha256(str(resolved).encode()).hexdigest()
                tracks.append({'id': 'local-' + identity, 'artist': artist or 'Unknown artist',
                               'title': title, 'url': str(resolved), 'start': 0,
                               'end': duration, 'duration': duration,
                               'fingerprint': f'{stat.st_size}:{stat.st_mtime_ns}'})
            except (OSError, ValueError, subprocess.SubprocessError):
                skipped.append(str(path.relative_to(root)))
        if not tracks:
            continue
        slug = re.sub(r'[^a-z0-9]+', '-', genre.name.lower()).strip('-') or 'genre'
        key = 'local-' + slug
        if key in sources:
            key += '-' + hashlib.sha256(genre.name.encode()).hexdigest()[:8]
        version = hashlib.sha256(json.dumps(tracks, sort_keys=True).encode()).hexdigest()
        sources[key] = {'label': 'Local · ' + genre.name, 'catalog_version': version,
                        'duration': sum(track['duration'] for track in tracks), 'tracks': tracks}
    if not sources:
        raise ValueError('No readable music found in genre subfolders')
    return {'root': str(root), 'genres': sources}, skipped
