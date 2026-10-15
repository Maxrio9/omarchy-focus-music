# Security review — 15 October 2026

This review covers the Rust backend, QML widgets, Lua normalization hook, launchers, local IPC, catalogue/local-file inputs, compact-bar edits, dependencies and release verification. It followed the marketplace report about unintended microphone fallback.

## Findings and fixes

| Area | Result |
| --- | --- |
| PipeWire target fallback | Fixed. Capture sets `node.dont-fallback=true`, `node.dont-reconnect=true`, `node.dont-move=true`, and `node.linger=false` before connecting. A missing or removed player target must fail closed rather than select the default microphone. |
| Capture identity and lifetime | Fixed. Only the player's playback stream and a positive `object.serial` are accepted; reusable registry IDs are not used as targets. Peer PID and node identity checks run periodically even with continuous input. The capture child is killed and reaped on exit. |
| Catalogue source protocols | Fixed. Catalogues accept HTTP(S) URLs or absolute local file paths/local file URIs, reject control characters and player/device pseudo-protocols, and reject existing non-regular files. The bundled catalogue passes this validation. |
| Resolver and media probing | Hardened. Online loudness resolution cannot redirect to a local file or another protocol. Local FFmpeg/ffprobe inputs allow only the file protocol; online analysis allows HTTP(S) and its transport/crypto protocols. mpv explicitly disables automatic user-script loading and unsafe playlists; the bundled normalization hook remains explicitly loaded. |
| Temporary files, runtime directories and logs | Hardened. JSON writes use unique, exclusively created mode-0600 temporary files and atomic replacement. Private directories are created mode 0700, checked for ownership and opened without following a final symlink. Lock files and player logs reject symlinks. |
| Compact-bar file paths | Hardened. Manifest entry paths must remain inside the canonical cloned-plugin directory, including after resolving symlinks. Other plugins and system-owned Omarchy sources are not edit targets. |

The filesystem findings concern predictable temporary paths, nonstandard/shared runtime locations and unusual cloned-plugin layouts. Standard installations already place runtime sockets inside a private user directory; this is not evidence of an existing remote compromise.

## Other surfaces reviewed

- Commands use argument arrays, not interpolated shell commands. Track titles, genre keys and URLs are not executable command text. The Lua hook also uses argument arrays.
- Player commands use structured JSON. The controller exposes a fixed command allowlist through a mode-0600 Unix socket inside a mode-0700 runtime directory. There is no TCP listener. RPC requests have a size limit and socket timeouts.
- Media integration uses the session bus and fixed MPRIS operations; `OpenUri` does not load arbitrary external requests. Track and genre labels use plain-text rendering, including the Omarchy Button component used by dropdown entries.
- The visualizer transforms samples into spectrum bands in memory. It has no feature to save or upload raw capture samples. This code review does not establish what happened on a previously running version.
- Release verification rebuilds both bundled architectures in a pinned environment, compares against the original committed digests and performs byte-for-byte comparisons without replacing the originals.
- Credential-pattern and private-path scans of tracked files and rebuilt binaries found no matches. The existing preview contains staged public music metadata.

## Verification

- Rust integration regressions cover continuous-input node loss, missing targets, missing/zero serials, unsafe catalogue protocols, online-to-local resolver redirection, runtime/lock symlinks, atomic JSON writes and symlink escapes from cloned plugins.
- `tests/pipewire.rs` uses a private PipeWire daemon, a private D-Bus session, WirePlumber's stock policy-only profile, a synthetic source and a null sink. No hardware monitors or host audio devices are used. It reproduces legacy fallback to the synthetic source, then verifies zero samples for a protected missing target and capture exit on target removal. It also checks the stream properties received by PipeWire.
- A dedicated CI job requires that real policy test in an isolated container. The normal integration run may skip it when PipeWire/WirePlumber 0.5+ are unavailable; the dedicated job may not. The local real-policy test passed with PipeWire 1.6.8 / WirePlumber 0.5.17.
- Formatting, strict Clippy, the integration suite, catalogue audit and release reproducibility are checked. The Wayland UI integration check is skipped in a headless session; no widget appearance or layout changes are part of this fix.
- `cargo-audit` 0.22.2 checked 131 locked dependencies against RustSec database commit `edb58f5974b83eb8302332821cb6d389777aa351` (14 October 2026): **zero known vulnerabilities and zero advisory warnings**. This is a dated result, not a permanent dependency guarantee.

## Scope and limits

This is a source review and targeted regression testing, not an independent security certification or a sandbox for media decoders. mpv, FFmpeg, yt-dlp, PipeWire, WirePlumber, Qt and the operating system remain external dependencies. Custom catalogues deliberately select files or network locations; import catalogues you trust. The plugin does not restrict HTTP(S) requests to public Internet addresses. Processes already able to modify this user's executable files or desktop configuration are outside the isolation boundary. Audio tests and executable checks run on x86-64; ARM64 binaries are cross-built and checked for reproducibility, not tested on ARM64 hardware.
