#!/usr/bin/env python3
"""Validate and report the bundled catalog without fetching audio."""
import json
from pathlib import Path

catalog = json.loads((Path(__file__).resolve().parents[1] / 'data/sources.json').read_text())
identities, segments = set(), set()
for genre, source in catalog.items():
    seconds = 0
    for track in source['tracks']:
        segment = (track['url'], track['start'], track['end'])
        if track['id'] in identities or segment in segments:
            raise SystemExit('Duplicate track: ' + track['artist'] + ' — ' + track['title'])
        if not 0 <= track['start'] < track['end'] or track['duration'] != track['end'] - track['start']:
            raise SystemExit('Invalid time boundary: ' + track['title'])
        identities.add(track['id'])
        segments.add(segment)
        seconds += track['duration']
    if seconds != source['duration'] or seconds < 86400:
        raise SystemExit('Invalid or insufficient duration: ' + genre)
    print(f'{source["label"]}: {len(source["tracks"]):,} tracks, {seconds / 3600:.1f} hours')
print('No duplicate metadata IDs or exact source segments. Audio fingerprints are not checked.')
