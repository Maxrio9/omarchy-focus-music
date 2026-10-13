#!/usr/bin/env python3
"""Add narrow-screen collapse to a user-owned clone of Omarchy's bar."""
import datetime
import json
import os
from pathlib import Path
import pwd
import shutil
import subprocess


MARKER = '// Focus Music compact bar'
RIGHT_ORIGINAL = '''  component RightModules: ModuleList {
    entries: root.layoutEntries("right")
    region: "right"
  }'''
RIGHT_COMPACT = '''  component RightModules: ModuleList {
    entries: root.layoutEntries("right")
    region: "right"
    // Focus Music compact bar: retain the widgets and their services while
    // clipping the right section during a center reveal on narrow screens.
    readonly property bool compactHidden: root.centerSectionRevealHeld
      && !root.centerHoverRevealSuppressed
      && (root.vertical || parent.width < 1600)
    opacity: compactHidden ? 0 : 1
    enabled: !compactHidden
    clip: true
    width: compactHidden && !root.vertical ? 0 : (item ? item.implicitWidth : 0)
    height: compactHidden && root.vertical ? 0 : (item ? item.implicitHeight : 0)
  }'''
HOVER_ORIGINAL = '          onHoveredChanged: root.setCenterSectionHovered(hovered)'
HOVER_COMPACT = '''          id: compactCenterHover
          onHoveredChanged: root.updateCenterHover(compactCenterHover)
          onPointChanged: root.updateCenterHover(compactCenterHover)'''
HOVER_FUNCTION = '''  // Only the actual center widgets trigger a reveal. The stock handler's
  // parent spans the bar; hovering the right-side icons must keep them usable.
  function updateCenterHover(handler) {
    var window = handler.parent.QsWindow.window
    var hit = false
    if (handler.hovered) {
      for (var i = 0; i < root.moduleSlots.length; i++) {
        var slot = root.moduleSlots[i]
        if (!slot || slot.region !== "center" || slot.width <= 0 ||
            !slot.visible || root.targetWindow(slot) !== window) continue
        var point = slot.mapFromItem(handler.parent, handler.point.position.x, handler.point.position.y)
        if (point.x >= 0 && point.y >= 0 && point.x < slot.width && point.y < slot.height) {
          hit = true
          break
        }
      }
    }
    root.setCenterSectionHovered(hit)
  }

'''


def patch_bar(source):
    if MARKER in source:
        return source
    if source.count(RIGHT_ORIGINAL) != 1 or source.count(HOVER_ORIGINAL) != 2:
        raise RuntimeError('This Omarchy bar version is not supported by compact-bar')
    source = source.replace(RIGHT_ORIGINAL, RIGHT_COMPACT)
    source = source.replace(HOVER_ORIGINAL, HOVER_COMPACT)
    return source.replace('  function setCenterSectionHovered(hovered) {',
                          HOVER_FUNCTION + '  function setCenterSectionHovered(hovered) {', 1)


def main():
    config_home = Path(os.environ.get('XDG_CONFIG_HOME') or str(Path.home() / '.config'))
    config = config_home / 'omarchy/shell.json'
    current = json.loads(config.read_text()).get('bar', {}).get('id', 'omarchy.bar')
    # Omarchy's native clone command currently uses HOME/.config, regardless
    # of XDG_CONFIG_HOME; avoid changing a different configuration by mistake.
    if config_home.resolve() != (Path.home() / '.config').resolve():
        raise RuntimeError('compact-bar requires Omarchy at the standard user config location')
    clone_id = (os.environ.get('USER') or pwd.getpwuid(os.getuid()).pw_name) + '.bar'
    if current not in ('omarchy.bar', clone_id):
        raise RuntimeError('A different custom bar is active; compact-bar leaves it unchanged')
    clone = config_home / 'omarchy/plugins' / clone_id
    backup = Path(os.environ.get('XDG_STATE_HOME') or str(Path.home() / '.local/state')) / 'focus-music/backups'
    backup /= 'compact-bar-' + datetime.datetime.now().strftime('%Y%m%d-%H%M%S-%f')
    backup.mkdir(parents=True, mode=0o700)
    shutil.copy2(config, backup / 'shell.json')
    if not clone.exists():
        native = Path(os.environ.get('OMARCHY_PATH') or '/usr/share/omarchy') / 'shell/plugins/bar/Bar.qml'
        patch_bar(native.read_text())  # Validate before creating/activating a clone.
        subprocess.run(['omarchy', 'plugin', 'clone', 'omarchy.bar'], check=True)
    bar_file = clone / 'Bar.qml'
    source = bar_file.read_text()
    updated = patch_bar(source)
    if updated != source:
        shutil.copy2(bar_file, backup / 'Bar.qml')
        bar_file.write_text(updated)
    subprocess.run(['omarchy', 'bar', 'use', clone_id], check=True)
    subprocess.run(['omarchy', 'restart', 'shell'], check=True)
    print('Compact bar enabled. To restore the standard bar: omarchy bar use omarchy.bar')


if __name__ == '__main__':
    main()
