# Building Focus Music

The backend and maintenance tools are Rust; QML provides the Omarchy interface and a small Lua hook forwards mpv events. The repository pins its compiler in `rust-toolchain.toml`.

Release binaries use a digest-pinned Rust Docker image on `linux/amd64`. With Docker installed, build both portable static binaries:

```bash
scripts/build-native-reproducible
```

This intentionally replaces the release binaries and `libexec/SHA256SUMS`. Builds use a fresh container with fixed source and Cargo paths, locked dependencies and Rust's pinned linker for both architectures. No host Cargo cache or credentials are mounted. Installation uses the bundled binary for the CPU and does not require Docker or a toolchain.

To verify a checkout without overwriting its binaries or checksums:

```bash
scripts/verify-native
```

CI runs this independently of the tests. It saves the original checksum manifest, builds into a temporary directory, checks the rebuilt files against those original digests, and compares both binaries byte-for-byte. A mismatch fails verification even if the committed binaries and their checksums agree with each other.

For local development, install Rust with rustup and run `scripts/build-native x86_64-unknown-linux-musl`. These host builds are not canonical release builds. `FOCUS_MUSIC_NATIVE_OUTPUT` selects an alternate output directory. `FOCUS_MUSIC_BINARY` selects a development binary; `FOCUS_MUSIC_PACKAGE` selects its package directory.

Run checks with:

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
scripts/audit-catalog
omarchy plugin validate .
```

Playback and UI tests use isolated players. Quickshell UI tests require an Omarchy Wayland desktop; real session migration requires mpv 0.38+. For manual experiments, isolate state through `FOCUS_MUSIC_CONFIG`, `FOCUS_MUSIC_LOCAL_CONFIG`, `FOCUS_MUSIC_PREFERENCES`, `FOCUS_MUSIC_STATE`, `FOCUS_MUSIC_RUNTIME`, and `FOCUS_MUSIC_CACHE`.
