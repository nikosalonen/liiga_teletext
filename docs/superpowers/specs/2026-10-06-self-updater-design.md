# Self-updater design

Date: 2026-10-06

## Goal

Let a user update with one command, `liiga_teletext --update`, instead of typing
`cargo install liiga_teletext`. The update should be fast on common platforms and
still work where no prebuilt binary exists.

## Decisions

- **Trigger:** explicit `--update` flag only. Nothing updates on its own, and
  there is no prompt in `--version`, `--once` or interactive mode.
- **Source of truth for "latest version":** crates.io `max_stable_version`, the
  same value the existing update check uses.
- **Two install paths:** download a prebuilt binary from the GitHub release when
  one exists for this platform; otherwise fall back to `cargo install`.
- **Prebuilt targets:**
  - `aarch64-apple-darwin`
  - `x86_64-apple-darwin`
  - `x86_64-unknown-linux-musl`
  - `aarch64-unknown-linux-musl`
  - `x86_64-pc-windows-msvc`
- **Implementation:** hand-rolled on the existing `reqwest` client, plus two new
  dependencies: `sha2` (checksum) and `self-replace` (swaps the running
  binary on Windows only; Unix uses `std::fs::rename` onto the canonicalized
  exe, because self-replace 1.5 follows only one symlink level). Rejected: the `self_update` crate (adds a second
  reqwest version, has no SHA-256 check, and makes the cargo fallback awkward)
  and cargo-dist + axoupdater (only updates installs that have a cargo-dist
  receipt, which existing `cargo install` users don't have).

## Release pipeline

A separate workflow, `.github/workflows/release-binaries.yml`, runs on `v*.*.*` tags and as a build-only dry run on PRs that touch it or `Cargo.lock`. A matrix `build` job uploads each binary as an artifact. A single `release` job then writes the `.sha256` files with `sha256sum` and uploads everything with the `gh` CLI, creating the release if it doesn't exist yet. Using one job means the matrix jobs don't race to create the release. No third-party action runs with the write token, and the third-party actions are pinned to commit SHAs. A final `publish` job then runs `cargo publish`, after the upload and the tests. crates.io is where `--update` finds new versions, so publishing last means a version never shows up before its binaries do.

Matrix:

| Target                       | Runner             | Extra setup                   |
| ---------------------------- | ------------------ | ----------------------------- |
| `aarch64-apple-darwin`       | `macos-latest`     | none                          |
| `x86_64-apple-darwin`        | `macos-latest`     | add rustup target             |
| `x86_64-unknown-linux-musl`  | `ubuntu-latest`    | `musl-tools`, `CC_*=musl-gcc` |
| `aarch64-unknown-linux-musl` | `ubuntu-24.04-arm` | `musl-tools`, `CC_*=musl-gcc` |
| `x86_64-pc-windows-msvc`     | `windows-latest`   | `AWS_LC_SYS_PREBUILT_NASM=1`  |

Each matrix entry:

1. Runs `cargo build --release --locked --target <target>`.
2. Copies the binary to `liiga_teletext-<target>` (`.exe` suffix on Windows).
3. Uploads the binary as a workflow artifact.

Assets are plain binaries, not archives, so the updater needs no tar or zip code.

**Resolved in the PR dry run:** `reqwest` uses rustls with `aws-lc-sys`,
which compiles C code. The musl targets need `musl-tools` (for `musl-gcc`).
On the arm runner the `cc` crate treats aarch64 musl as a cross-compile and
looks for `aarch64-linux-musl-gcc`, which `musl-tools` does not ship, so the
build sets `CC_<target>=musl-gcc` for both musl targets. Windows needs NASM for
`aws-lc-sys`. The build sets `AWS_LC_SYS_PREBUILT_NASM=1`, so `aws-lc-sys` uses
the NASM objects that ship in the crate. (It first used `ilammy/setup-nasm`,
which has no Node 24 release.) `cargo-zigbuild` was not needed.

## App changes

### CLI (`src/cli.rs`, `src/main.rs`)

- Add `--update` (`pub update: bool`) under `help_heading = "Info"`, with help
  text "Update liiga_teletext to the latest version".
- Add `args.update` to `is_noninteractive_mode`.
- In `main.rs`, handle `args.update` right after `args.version`, calling
  `commands::handle_update_command()`.
- Reject `--update` combined with any other action flag in
  `commands::validate_args` (same style as the existing checks).

### Version lookup (`src/version.rs`)

- Split `check_latest_version` into:
  - `fetch_latest_version(crates_io_base: &str) -> Result<Version, AppError>`,
    which returns errors instead of printing them.
  - `check_latest_version() -> Option<String>`, which keeps its current
    behaviour for the startup check by wrapping the new function.
- Change the "Update available! Run:" hint in `print_version_info` from
  `cargo install liiga_teletext` to `liiga_teletext --update`.

### Updater (`src/self_update.rs`, new)

Plain functions, with base URLs passed in so tests can point them at wiremock:

- `asset_name() -> Option<&'static str>`: maps `std::env::consts::{OS, ARCH}` to
  the asset name. Returns `None` for any platform not in the matrix. Linux always
  maps to the musl build.
- `fetch_checksum(client, release_base, version, asset) -> Result<Option<[u8; 32]>, AppError>`:
  GETs `<release_base>/v<version>/<asset>.sha256`. A 404 gives `Ok(None)`. A
  body that doesn't parse as 64 hex characters is an error.
- `download_and_verify(client, url, expected, dest_dir) -> Result<PathBuf, AppError>`:
  streams the asset into a temp file in `dest_dir`, computes SHA-256 while
  writing, and fails if the digest doesn't match. On Unix it sets mode `0o755`.
- Cargo fallback check (inside `Updater::run`): allowed only when `cargo` is
  on PATH **and** the running binary is `liiga_teletext` directly in cargo's
  bin folder (`$CARGO_HOME/bin`, else `~/.cargo/bin`). Without the second check,
  `cargo install` would put a new copy in `~/.cargo/bin` and leave the running
  one, for example in `/usr/local/bin`, unchanged. Always false on Windows,
  because `cargo install` cannot overwrite the running `.exe`.
- `run_update(...) -> Result<UpdateOutcome, AppError>`: the flow below.
  `UpdateOutcome` is an enum: `AlreadyLatest(Version)`,
  `Replaced { from: Version, to: Version }` (prebuilt binary swapped in) and
  `InstalledWithCargo { to: Version }`. "Manual steps needed" is an
  `Err(AppError::SelfUpdate(..))`, so the process exits non-zero.

Release downloads use the direct URL
`https://github.com/nikosalonen/liiga_teletext/releases/download/v<version>/<asset>`.
This avoids the GitHub REST API and its 60-requests-per-hour limit for clients
that aren't logged in. The download client uses a 10-second connect timeout and a 30-second read timeout, with no limit on the whole request, so a slow but steady download keeps going while a stalled one still fails. The
existing 10-second timeout is for small JSON requests.

### Flow

```
fetch latest version from crates.io
  error            → exit with error "could not check for updates: ..."
  latest <= current → print "already up to date (X)", exit 0

asset_name() is Some?
  yes → fetch_checksum
          Some(hash) → download_and_verify into the exe's folder
                         mismatch → abort with error, temp file removed, NO fallback
                         ok       → swap in temp, remove temp (Unix: std::fs::rename onto the
                                    canonicalized exe; Windows: self_replace)
                                    print "updated X → Y", exit 0
          None (404)  → go to fallback ("binaries for Y are not published yet")
          network err → go to fallback (could not connect, or the download broke off)
          HTTP error status (403, 429, 5xx) → exit with error, NO fallback
  no  → go to fallback ("no prebuilt binary for this platform")

fallback:
  fallback allowed → run `cargo install liiga_teletext --locked --version <latest>`
                           stdout/stderr pass through to the terminal
                           exit code non-zero → error
  otherwise        → print manual steps, exit with error:
                     no prebuilt for platform → "run cargo install liiga_teletext --locked"
                     binary is cargo's copy   → "try again later, or run cargo install ..."
                     anything else            → "try again later, or download it from
                                                 <release page>/tag/v<latest>"
```

Disk errors (for example a root-owned `/usr/local/bin`) surface as an
error that names the binary's folder and, for permission errors, suggests re-running with the needed
permissions. The updater never calls `sudo` itself.

A checksum mismatch never falls back to cargo. A wrong checksum means a broken
or tampered download, so the user should see it.

### Errors (`src/error.rs`)

Add one variant: `SelfUpdate(String)` with message `"Update failed: {0}"`.
Network errors keep using the existing `ApiFetch` variant through `?`, and
`unavailable_or_error` sorts them into "fall back" (no HTTP status) or "stop"
(an HTTP error status). Disk errors in the update become `SelfUpdate` with the
folder named.

### Output

Plain text on stdout. No full-screen UI. Example:

```
Current version: 0.27.0
Latest version:  0.28.0
Downloading liiga_teletext-aarch64-apple-darwin ...
Checksum OK
Updated 0.27.0 → 0.28.0
```

## Testing

Unit tests in `src/self_update.rs`:

- `asset_name` for each supported `(OS, ARCH)` pair and one unsupported pair.
  The mapping is a pure helper `asset_name_for(os, arch)`, so it can be tested on
  any host.
- Checksum parsing: plain digest, `sha256sum` format, uppercase hex, bad length,
  non-hex input.
- Cargo-bin path check (`is_inside_cargo_bin`), using a helper that takes the
  exe path and cargo bin folder as arguments (no PATH or env changes in tests).

Integration-style tests with `wiremock` and `tempfile`:

- `fetch_checksum`: 200 → digest. `try_prebuilt`: 404 → `Unavailable`;
  403/429/5xx, a bad checksum file or a mismatch → error, no file left behind.
- `Updater::run` (the `run_update` flow) against wiremock with a fake `Cargo`:
  up to date, crates.io failure, mismatch and HTTP errors never run cargo,
  404 falls back only for cargo installs, the release page link otherwise, and
  a symlinked binary replaces the real file.
- `download_and_verify`: matching digest → file written with correct bytes;
  mismatch → error and no file left behind.
- `fetch_latest_version` parses a crates.io response and errors on a bad body.

The Unix swap (`replace_binary`) is unit-tested, including through a symlink. The Windows swap (`self_replace`) is not unit-tested. It is checked by hand
on Windows once the first release with binaries is out.

## Docs

- README "Installation": add a "Prebuilt binaries" subsection pointing to the
  GitHub releases page, and an "Updating" subsection for `liiga_teletext --update`.
- CLAUDE.md: add `self_update.rs` to Module Responsibilities and a "Releases" note
  about the `release-binaries.yml` workflow.

## Out of scope

- Automatic or prompted updates.
- Signature checking beyond SHA-256 from the same release. Both files come from
  the same GitHub release, so this protects against corrupted downloads, not
  against a compromised release.
- Shell or PowerShell install scripts, Homebrew, or cargo-binstall metadata.
- Rolling back to the previous version.
