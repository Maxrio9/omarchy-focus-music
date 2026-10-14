"""Validated, user-owned display preferences."""
import json
from pathlib import Path

DEFAULTS = {'barDisplay': 'banner', 'placement': 'left', 'bannerWidth': None,
            'visualizerWidth': 100, 'otherMedia': 'pause-resume'}
CHOICES = {'barDisplay': ('banner', 'visualizer', 'icon'),
           'placement': ('left', 'clock-right', 'right'),
           'otherMedia': ('pause-resume', 'duck-strong', 'duck-gentle', 'none')}


def validate(changes):
    if not isinstance(changes, dict) or set(changes) - set(DEFAULTS):
        raise ValueError('Unknown display setting')
    for key, value in changes.items():
        if key in CHOICES and value not in CHOICES[key]:
            raise ValueError('Invalid ' + key)
        if key in ('bannerWidth', 'visualizerWidth') and value is not None and (type(value) is not int or not 80 <= value <= 500):
            raise ValueError('Width must be between 80 and 500')
        if key == 'visualizerWidth' and value is None:
            raise ValueError('Visualizer width requires an integer')
    return changes


def read(path, shell_path=None, plugin_id='maxrio9.focus-music'):
    settings = dict(DEFAULTS)
    try:
        saved = json.loads(path.read_text())
        if not isinstance(saved, dict):
            raise ValueError('Settings must be an object')
        for deprecated in ('popupVisualizer', 'volumeReactive'):
            saved.pop(deprecated, None)
        settings.update(validate(saved))
        return settings
    except (OSError, ValueError):
        pass
    if shell_path:
        try:
            layout = json.loads(shell_path.read_text())['bar']['layout']
            for region, entries in layout.items():
                for index, entry in enumerate(entries):
                    if isinstance(entry, dict) and entry.get('id') == plugin_id:
                        if region == 'right':
                            settings.update(placement='right', barDisplay='icon')
                        elif region == 'center':
                            clock = next((i for i, item in enumerate(entries)
                                          if isinstance(item, dict) and item.get('id') == 'omarchy.clock'), index + 1)
                            settings['placement'] = 'left' if index < clock else 'clock-right'
                        settings['bannerWidth'] = entry.get('bannerWidth')
        except (OSError, ValueError, KeyError):
            pass
    return settings
