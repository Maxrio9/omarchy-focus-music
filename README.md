# Focus Music for Omarchy

Instrumental background music, one click from your desktop bar. Pick a genre, see the artist and track name, and control playback with your mouse.

![Focus Music bar and genre selector](docs/preview.png)

## Features

- Five instrumental genres, each with more than 24 hours of catalogued music.
- Shuffled queues. Choosing a genre starts a random track instead of resuming.
- A fixed-width artist/track banner that scrolls continuously in one direction, pausing at the beginning.
- A themed hover popup with a genre dropdown.
- Background audio through mpv, streamed from YouTube or played from your own local genre folders.
- Background track loudness leveling with peak-safe fixed gain, preserving the dynamics inside each piece.
- No audio files bundled or permanently downloaded.

| Gesture on the banner | Action |
| --- | --- |
| Hover | Show the genre and playback popup |
| Left-click | Pin / unpin the popup |
| Right-click | Pause / resume |
| Middle-click | Next track |
| Mouse wheel | Adjust music volume by 5% |

Right-click, middle-click, and scrolling also work anywhere over the open popup. The Play/Pause label changes immediately and then reconciles with the player.

Music starts at 20% player volume and remembers volume changes. It does not change your system volume.

## Install

Requires **Omarchy 4's Quickshell bar** with the `qs.Commons` and `qs.Ui` components (`PopupCard`, `Dropdown`, and the bar buttons), Python 3.8+, and **mpv 0.38+**. Streaming requires a recent yt-dlp; local indexing and loudness measurement require FFmpeg. Tested on Omarchy 4.0.4 with mpv 0.41. Other Omarchy versions must provide the same shell UI API. A plain Hyprland installation or a Waybar-based Omarchy bar can use the command-line player; the QML widget requires the Omarchy shell. `notify-send` is optional and provides error notifications.

Install any missing dependencies:

```bash
omarchy pkg add python mpv yt-dlp ffmpeg libnotify
```

Then install and enable the plugin:

```bash
omarchy plugin add https://github.com/Maxrio9/omarchy-focus-music --enable
```

Place it left of the date and time:

```bash
omarchy bar move maxrio9.focus-music --section center --before omarchy.clock
```

Alternatively right of the date and time:

```bash
omarchy bar move maxrio9.focus-music --section center --after omarchy.clock
```

The plugin also provides `focus-music place left` and `focus-music place right` through its controller:

```bash
~/.config/omarchy/plugins/maxrio9.focus-music/bin/focus-music place right
```

For narrow or portrait screens, optional compact mode hides the right-side icons while the center widgets or music banner are revealed. They return when the pointer leaves the bar; approaching the icons directly keeps them available. This creates a user-owned clone of the standard Omarchy bar, preserving system files:

```bash
~/.config/omarchy/plugins/maxrio9.focus-music/bin/focus-music compact-bar
```

Restore the standard bar with `omarchy bar use omarchy.bar`. Compact mode supports Omarchy 4's standard bar and leaves a different custom bar unchanged.

If your shell retains an older widget after an update, run `omarchy restart shell`. Playback runs separately from the shell.

## Catalog

| Genre | Tracks | Catalogued playback |
| --- | ---: | ---: |
| Lo-fi hip-hop | 4,339 | 178.3 hours |
| Lo-fi jazz / jazzhop | 886 | 37.8 hours |
| Calm café jazz | 677 | 30.7 hours |
| Gentle classical piano | 477 | 35.3 hours |
| Gentle orchestra & chamber music | 728 | 55.8 hours |

Sources include [Lofi Girl](https://www.youtube.com/@LofiGirl), [Chillhop Music](https://www.youtube.com/@ChillhopMusic), [Cafe Music BGM channel](https://www.youtube.com/@cafemusicbgmchannel), and [HALIDONMUSIC](https://www.youtube.com/@HALIDONMUSIC). Original collection URLs are in [data/collections.json](data/collections.json). For classical music, the banner credits the composer where performer information is unavailable.

The catalog contains URLs, credits, durations and start/end boundaries. Those boundaries turn long mixes into individual skippable tracks. Online playback streams audio into mpv's memory buffer and requires an internet connection.

Duplicate metadata IDs and identical URL/start/end segments are removed across all five genres. This is a metadata audit, not audio fingerprinting: differently named uploads or different performances of the same composition may overlap. Catalogued hours describe the source listings; unavailable or removed videos can reduce playable hours. Queues repeat after all queued entries have been traversed, and skipped tracks count as traversed.

## Add your own music

### Local folders and offline playback

Supply your own music files and organize them into genre subfolders. You must obtain or download the files yourself; the plugin does not download music for offline use.

```text
Focus/
├── Piano/
│   ├── Artist - Track.flac
│   └── Another track.mp3
├── Jazz/
│   └── Album/
│       └── Track.ogg
└── Lo-fi hip-hop/
    └── Track.m4a
```

Point the player at the parent folder:

```bash
~/.config/omarchy/plugins/maxrio9.focus-music/bin/focus-music local "$HOME/Music/Focus"
```

Each immediate subfolder becomes a **Local · Genre** entry in the dropdown, alongside the online genres. Nested album folders are scanned too. The dropdown refreshes automatically. Tracks use embedded artist/title tags, or `Artist - Track` filenames when tags are absent. Common formats include MP3, FLAC, M4A, Ogg, Opus, and WAV; FFmpeg supplies the duration and tag reader.

Choose a local genre in the popup, or use its identifier from the import command:

```bash
~/.config/omarchy/plugins/maxrio9.focus-music/bin/focus-music play local-piano
```

Local genres play directly from those files without an internet connection or yt-dlp. Shuffle, Previous/Next, volume, the track banner, and loudness leveling work for local files too. Local playback requires mpv; indexing and loudness leveling require FFmpeg. The bundled 24-hour totals apply to the online catalog; local totals reflect the files you supply.

Run the `local` command again after adding, removing, or changing files. It replaces the local index without moving or changing the music, and reports files it could not read. The index stays in `${XDG_CONFIG_HOME:-~/.config}/focus-music/local-sources.json`; it is not stored in the plugin repository. One parent library is indexed at a time. To remove local entries, stop local playback and remove that index file. The dropdown then refreshes automatically.

### Custom online catalogs for people or agents

To add your own online tracks, copy `data/sources.json` to `${XDG_CONFIG_HOME:-~/.config}/focus-music/sources.json` and edit that copy. A custom catalog replaces the bundled online catalog, so copying first preserves the existing genres. Local folder genres are added separately. The popup refreshes when the catalog changes.

A person or agent can add a genre or append tracks using this structure:

```json
{
  "my-piano": {
    "label": "My piano collection",
    "catalog_version": "my-piano-v1",
    "duration": 300,
    "tracks": [
      {
        "id": "my-piano-unique-track-id",
        "artist": "Artist",
        "title": "Track title",
        "url": "https://www.youtube.com/watch?v=VIDEO_ID",
        "start": 0,
        "end": 300,
        "duration": 300
      }
    ]
  }
}
```

Use a unique ID for every track and real artist/title credits. Each track's `duration` must equal `end - start`; each genre's `duration` must equal the sum of its tracks. Start/end boundaries can select an individual piece within a longer mix. Change `catalog_version` when changing track membership or order so old queue indices cannot point at the wrong track. Keep the JSON valid, avoid duplicate IDs or identical URL/start/end segments, and choose instrumental tracks if you want to maintain the plugin's focus-music selection. User catalogs and local indexes belong in the user configuration directory, outside a Git checkout.

## Loudness leveling

The player measures each complete track in the background using FFmpeg's [EBU R128 scanner](https://ffmpeg.org/ffmpeg-filters.html#ebur128). It aims for −23 LUFS and calculates one gain adjustment for the whole piece, limited by a −2 dB true-peak ceiling and a maximum 12 dB boost. It does not compress the recording or continuously boost quiet passages. Quiet introductions and climaxes retain their contrast. A recording with unusually large dynamics may stay below the average target because peak headroom takes priority.

On the first listen, measurement streams the track once more for analysis. Music keeps playing while it runs; the measured gain is introduced with a three-second transition and then held steady. The next two queued tracks are checked ahead of playback. Local files are measured directly without streaming. Only numeric loudness/peak measurements are cached under `${XDG_CACHE_HOME:-~/.cache}/focus-music/loudness/`, not audio or signed streaming URLs. This uses additional bandwidth when a track has not been measured before. If analysis fails, playback continues at its normal level.

To disable measurement for a new player instance, set `FOCUS_MUSIC_NORMALIZE=0` before launching it. The visible volume percentage is your player-volume setting, separate from the per-track gain. Stop the independent player from the command line before starting a new instance if you need to change its environment.

## Customize

Set the banner width in Omarchy's logical size units:

```bash
omarchy bar set maxrio9.focus-music bannerWidth 180 --json
```

The default is 180, with a supported range of 80–500. The fixed width keeps neighboring widgets steady when the track changes. The preview above is captured from this same popup using a sample track; colors and corner shapes follow your Omarchy theme.

The plugin uses its bundled catalog by default. Put a custom catalog at `${XDG_CONFIG_HOME:-~/.config}/focus-music/sources.json` to override it. Each genre has a `label`, `catalog_version`, total `duration`, and a `tracks` array; each track supplies a unique `id`, `artist`, `title`, `url`, `start`, `end`, and `duration`. Change `catalog_version` whenever you change the track ordering or membership.

State is saved under `${XDG_STATE_HOME:-~/.local/state}/focus-music/`. The private mpv socket and playback log live under `$XDG_RUNTIME_DIR/focus-music/`.

The player also works from the command line on Linux desktops:

```bash
~/.config/omarchy/plugins/maxrio9.focus-music/bin/focus-music genres
~/.config/omarchy/plugins/maxrio9.focus-music/bin/focus-music play piano
~/.config/omarchy/plugins/maxrio9.focus-music/bin/focus-music toggle
~/.config/omarchy/plugins/maxrio9.focus-music/bin/focus-music previous
~/.config/omarchy/plugins/maxrio9.focus-music/bin/focus-music skip
~/.config/omarchy/plugins/maxrio9.focus-music/bin/focus-music stop
```

For testing or separate player instances, override `FOCUS_MUSIC_CONFIG`, `FOCUS_MUSIC_LOCAL_CONFIG`, `FOCUS_MUSIC_STATE`, `FOCUS_MUSIC_RUNTIME`, and `FOCUS_MUSIC_CACHE`.

## Update or remove

```bash
omarchy plugin update maxrio9.focus-music
```

Before removing the plugin, stop its independent audio player:

```bash
~/.config/omarchy/plugins/maxrio9.focus-music/bin/focus-music stop
omarchy plugin remove maxrio9.focus-music
```

Saved volume and queue state remain in your user state directory.

## Development

The controller uses only Python's standard library. Offline tests check catalog durations and duplicate entries, shuffle consumption, segment boundaries, portable paths, clean installs with spaces and Unicode, unset/empty/custom XDG directories, track credits, mpv's JSON IPC framing, local folder/tag indexing, offline loudness measurement, both clock placements, the compact-bar patch, and track-wide normalization. Waveform tests verify that differently mastered levels converge while quiet/loud contrast stays intact.

```bash
python -m unittest discover -s tests -v
python scripts/audit-catalog.py
omarchy plugin validate .
```

For a playback smoke test, point `FOCUS_MUSIC_RUNTIME` and `FOCUS_MUSIC_STATE` at temporary paths and set the player volume to zero before playing. QML uses Omarchy's `qs.Commons` and `qs.Ui` modules, so test it in the Omarchy shell.

## License and music credits

Plugin code is [MIT licensed](LICENSE). The music belongs to its respective artists and rights holders. This repository distributes source metadata and player code, not audio recordings. Support the artists and channels through the source links.
