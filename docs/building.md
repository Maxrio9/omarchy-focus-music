# Building Focus Music

The backend and maintenance tools are Rust; QML provides the Omarchy interface and a small Lua hook forwards mpv events. The repository pins its compiler in `rust-toolchain.toml`.

Install Rust with rustup, then build the portable static binaries:

```bash
scripts/build-native x86_64-unknown-linux-musl aarch64-unknown-linux-musl
```

The script strips contributor paths from binaries and updates `libexec/SHA256SUMS`. Installation uses the bundled binary for the CPU and does not install a toolchain. `FOCUS_MUSIC_BINARY` can select a development binary; `FOCUS_MUSIC_PACKAGE` selects its package directory.

Run checks with:

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
scripts/audit-catalog
omarchy plugin validate .
```

Playback and UI tests use isolated players. Quickshell UI tests require an Omarchy Wayland desktop; real session migration requires mpv 0.38+. For manual experiments, isolate state through `FOCUS_MUSIC_CONFIG`, `FOCUS_MUSIC_LOCAL_CONFIG`, `FOCUS_MUSIC_PREFERENCES`, `FOCUS_MUSIC_STATE`, `FOCUS_MUSIC_RUNTIME`, and `FOCUS_MUSIC_CACHE`.
