# Performance review

## Rust backend in 2.0.0

Measured on the same desktop and paused mpv stream, comparing the previous 1.7.6 Python backend with the bundled x86-64 Rust release build. Each helper ran for a three-second warmup and an eight-second CPU sample. Memory came from `/proc/PID/smaps_rollup`; CPU came from `/proc/PID/stat`. CPU percentages use **one logical core as 100%**, with approximately 0.125 percentage-point sampling resolution. These short observations describe this workload, not guaranteed limits.

The bridges ran separately in private D-Bus sessions with copied state and preferences. The visualizers captured the same player's output. Measurements did not change playback, system volume, or the production bridge's media-key ownership.

| Helper | CPU, one core | Resident RAM (RSS) | Proportional RAM (PSS) |
| --- | ---: | ---: | ---: |
| Python bridge | 0.375% | 41.17 MiB | 25.25 MiB |
| Rust bridge | 0.500% | 11.63 MiB | 11.62 MiB |
| Python visualizer | 0.875% | 26.86 MiB | 20.35 MiB |
| Rust visualizer | 0.750% | 5.60 MiB | 5.59 MiB |

Combined helper RSS dropped from **68.0 to 17.2 MiB**, about **75% less**. The visualizer retains its frequency-band calculations, frame rate, volume response, and separate bar/popup layouts. It shares one native FFT between both displays. The native bridge includes a private command server with the catalog already loaded.

The visualizer and its PipeWire capture subprocess run only when visible during playback. The bridge runs while the widget is enabled. PipeWire's `pw-cat` capture helper remains a separate native process; it previously measured about 9 MiB RSS and under the 0.13% CPU sampling resolution.

The player was paused during this final comparison, so the CPU values above describe paused helpers, not active FFT work. An earlier active-playback comparison, before adding the cached command server, measured the Python visualizer at 3.375% and the Rust visualizer at 0.125% of one core; helper RSS was again approximately 68 MiB versus 17 MiB. These distinct workloads should not be combined into a single CPU reduction claim.

### Controller response

Thirty sequential status commands per implementation included launcher and process startup. The cached Rust measurement used the running native bridge; the cold Rust measurement had no bridge.

| Controller | Wall time per call | Child CPU time per call |
| --- | ---: | ---: |
| Python CLI | 48.07 ms | 47.15 ms |
| Rust CLI | 39.56 ms | 39.22 ms |
| Rust cached CLI | 10.76 ms | 9.43 ms |

Normal widget actions use the cached catalog through a private Unix socket instead of reparsing it in each new process. CLI control remains available when the widget is absent through the standalone fallback. The bridge sends changes immediately after acknowledged actions and publishes periodic status heartbeats, so the widget does not need repeated controller startup for routine updates. First-time streaming resolution, file indexing, and loudness scans have their own costs and are outside this status benchmark.

### What remains

mpv still decodes and buffers the audio. Earlier streaming observations measured roughly 173–178 MiB RSS for mpv; that component was not rewritten or reduced by this port. FFmpeg handles loudness analysis, yt-dlp resolves online sources, and PipeWire captures the player's output. QML renders inside the existing native Qt/Quickshell desktop shell. Its total resources cannot fairly be attributed to this widget because it hosts the rest of the desktop too.

The Rust port therefore saves roughly 51 MiB of dedicated helper RSS in this sample, rather than removing the player's entire memory use. Excluding the shared desktop shell and transient analysis, adding the earlier mpv and capture samples gives roughly 199–204 MiB dedicated RSS with visualization. This is an estimate using components measured at different times, not a new simultaneous total measurement. PSS apportions shared pages and is usually more useful than summing RSS.

No reduction in audio buffers, visualization rate, or UI features was needed. Short-lived analysis workers can still add CPU, bandwidth, and memory while a track is measured. Different codecs, caches, track transitions, screen rendering, and system load will change results.

## Verification and portability

Rust integration tests cover queue and segment behavior, state/cache compatibility, settings, private command routing, media keys and other-player handoffs, local indexing, whole-track normalization with preserved dynamics, subprocess cancellation and cleanup, and an actual mpv session upgrade. The QML controls are also exercised through Quickshell on an Omarchy Wayland desktop.

Bundled static musl binaries support x86-64 and ARM64 Linux. The x86-64 build was exercised on the development desktop. The ARM64 build was cross-compiled and inspected, but has not been tested on ARM hardware. Checksums accompany both binaries. A source build is supported through the pinned compiler and build script.
