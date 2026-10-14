"""Validated, user-owned display preferences."""
import json
from pathlib import Path

DEFAULTS = {'barDisplay': 'banner', 'placement': 'left', 'popupVisualizer': True,
            'volumeReactive': True, 'bannerWidth': None}
CHOICES = {'barDisplay': ('banner', 'visualizer', 'icon'),
           'placement': ('left', 'clock-right', 'right')}


def validate(changes):
    if not isinstance(changes, dict) or set(changes) - set(DEFAULTS):
        raise ValueError('Unknown display setting')
    for key, value in changes.items():
        if key in CHOICES and value not in CHOICES[key]:
            raise ValueError('Invalid ' + key)
        if key in ('popupVisualizer', 'volumeReactive') and not isinstance(value, bool):
            raise ValueError(key + ' must be true or false')
        if key == 'bannerWidth' and value is not None and (type(value) is not int or not 80 <= value <= 500):
            raise ValueError('Banner width must be between 80 and 500')
    return changes


def read(path, shell_path=None, plugin_id='maxrio9.focus-music'):
    settings = dict(DEFAULTS)
    try:
        settings.update(validate(json.loads(path.read_text())))
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
