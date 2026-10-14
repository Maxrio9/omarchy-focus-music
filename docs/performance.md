# Performance review

Measured on the development desktop during steady-state online playback, using eight-second CPU samples from `/proc/PID/stat` and memory readings from `/proc/PID/smaps_rollup`. These are short observations, not guaranteed limits. CPU percentages use **one logical core as 100%**.

| Process | CPU, one core | Resident RAM (RSS) | Proportional RAM (PSS) |
| --- | ---: | ---: | ---: |
| mpv, current streaming playback | 0.8–0.9% | 173–176 MiB | 126–129 MiB |
| MPRIS media bridge | 0.5–0.6% | 42 MiB | 26 MiB |
| Optimized live visualizer | 3.1% | 26 MiB | 20 MiB |
| PipeWire capture helper | under 0.13% sampling resolution | 9 MiB | 3 MiB |

The visualizer and capture helper run only when visible during playback. Dedicated processes therefore used approximately **215–218 MiB RSS without visualization** and **250–253 MiB with visualization** in these samples. PSS totals were about 153–156 MiB and 176–179 MiB respectively; PSS apportions shared library pages rather than counting them fully in every process.

These totals exclude the existing shared Omarchy shell, UI rendering, and transient helpers. The shell also hosts the rest of the desktop bar, so assigning its entire memory or CPU use to Focus Music would be misleading. Short-lived controller calls and first-time FFmpeg/yt-dlp loudness analysis can add CPU and memory bursts. Streaming buffers, codecs, track changes, other media players, and system load affect results. An idle mpv instance can retain buffers from earlier playback.

## Improvements in 1.7.2

The visualizer previously performed the same 1,024-sample FFT twice per frame. A shared transform now supplies both the quickbar bands and the five popup bands. A 300-frame CPU-time benchmark dropped from **2.26 to 1.14 ms per frame**; live helper CPU fell from **5.5% to 3.1% of one core**. Spectrum regression tests confirm the same band results.

Status polling also started Python and parsed the full catalog on every refresh. Twenty representative calls averaged **47 ms of CPU time each**, equivalent to roughly **2.4% of one core** at two-second polling intervals or **11.8%** at the former 400 ms popup interval. The widget now consumes status changes and heartbeats from its existing media bridge, eliminating those repeated calls in normal operation. Polling remains available if the bridge is absent or stops delivering status. The live CPU table was measured before deploying this status-stream change; the visualizer optimization was included.

## Would Rust help?

A Rust implementation of the media bridge and spectrum helper could reduce interpreter memory and FFT CPU costs. That is an architectural expectation, not a measured Rust result. It would not remove mpv's playback buffers, FFmpeg's analysis work, or the shared Qt/Quickshell renderer, which already use native code. Rewriting the complete plugin would also add packaging, binary distribution, and maintenance work.

The immediate gains came from removing duplicate computation and repeated process startup. Keep the QML interface and mpv playback. If further savings are needed, profile a small native spectrum helper or a single persistent backend first, then compare real playback measurements before committing to a full rewrite.

## Estimated next improvements

These estimates keep mpv, the current visualization rate, and the QML interface. They describe total dedicated-process RSS and steady-state CPU with the visualizer active, excluding the shared desktop shell and analysis bursts. They are planning targets, not measurements of completed alternative implementations.

| Approach | Estimated total RSS | Estimated CPU, one core |
| --- | ---: | ---: |
| Current measured implementation | about 254 MiB | about 4.8% |
| Further pure-Python cleanup | 225–235 MiB | 3.5–4.3% |
| Python backend with a native FFT routine | 225–240 MiB | 2–3% |
| One persistent Rust backend, retaining mpv/QML | 195–210 MiB | 1–2% |

The Python memory target comes from removing duplicate catalogs, loading less controller code in the visualizer, and potentially sharing one persistent backend. Isolated import measurements found approximately **11 MiB extra RSS per helper** from importing the full controller/catalog. FFT-only imports used 13.7 MiB RSS versus 24.8 MiB with the controller; Gio imports used 28.5 MiB versus 39.8 MiB with it. These are startup measurements and do not include live audio or all bridge objects.

A temporary benchmark retained Python for windowing and band aggregation while calling an installed native FFT library through ctypes. It measured **1.20 ms/frame** for the current Python transform path versus **0.23 ms/frame** for the native path, with matching band results. This demonstrates a possible CPU gain without changing the backend language; it is not an installed plugin change or a commitment to a particular dependency. The experiment used the [FFTW real-data transform API](https://www.fftw.org/fftw3_doc/One_002dDimensional-DFTs-of-Real-Data.html).

The Rust estimate assumes a compact, unified backend using direct D-Bus bindings rather than retaining the Python/Gio process. It could replace roughly 68 MiB of Python helper RSS with a backend budget of roughly 8–20 MiB, while retaining mpv and the capture helper. No Rust prototype has been measured, so implementation choices could move those numbers substantially.

mpv currently accounts for about 178 MiB RSS. Numeric cache accounting at the later inspection showed about **16 MiB** of buffered packet data; the configured maximum is a ceiling, not an allocation. A smaller cache can limit peaks but should not be assumed to recover most of mpv's current memory. [mpv's cache documentation](https://mpv.io/manual/stable/#cache) describes how time and byte limits interact. Playback memory deserves a separate profile before promising larger total-RAM savings.

The practical first step is the Python memory cleanup plus a native transform. A Rust backend offers additional memory savings and less interpreter overhead, with higher implementation and distribution effort.
