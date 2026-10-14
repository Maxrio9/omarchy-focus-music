# Updating from 1.x to the Rust backend

Configuration, saved volume, queues, and loudness measurements keep their existing locations. No compiler is needed: the plugin includes native Linux binaries.

After updating, replace a running player's old helper hook once:

```bash
~/.config/omarchy/plugins/maxrio9.focus-music/bin/focus-music restart-player
omarchy restart shell
```

This briefly restarts mpv while preserving the indexed queue, current position, pause state, volume, and generation. If you created your own launcher that explicitly runs `python3`, remove that prefix and execute `bin/focus-music` directly.
