# Launcher ↔ game contract v0

The ReRAC launcher (this repo, `re-rac/rerac-launcher`) and the ReRAC game repo (`re-rac/rerac`) both implement exactly this. Types:
`src-tauri/src/contract.rs` (Rust) and `src/backend/contract.ts` (TypeScript). Change all three
together.

## Version

A **version** is one game build, official or (later) a mod. It is a folder
`versions/<source>/<version>/` that contains `rerac-manifest.json`:

```json
{
  "schema": 1,
  "name": "rerac",
  "version": "0.1.0",
  "game": "rac1",
  "runtime": "<relative path to game binary>",
  "extractor": "<relative path to extractor binary>",
  "supported_discs": ["SCUS_971.99"],
  "data_format": 1
}
```

- `runtime` and `extractor` are relative to the version folder and may not leave it (no `..`,
  no absolute paths). The launcher rejects the manifest otherwise.
- The launcher accepts `schema` 1 only and requires `name` = `rerac`.
- The Development source points at a local folder with this manifest (for example a dev build
  folder). It is used in place; nothing is copied.

## Extractor CLI (`rerac-extract`)

```
rerac-extract identify --iso <path> --json
rerac-extract extract  --iso <path> --out <game_data_dir> [--ntsc-only] --json
rerac-extract verify   --out <game_data_dir> --json
```

- `identify` identifies the disc only.
- `extract` does the full extraction.
- `verify` re-hashes against the built-in size/SHA-1 table.

With `--json`, stdout is JSON lines, one object per line:

| `type` | Fields |
|---|---|
| `progress` | `stage`: `identify` \| `copy` \| `verify`; `done`, `total` (bytes); `file` (path) |
| `info` | `message` |
| `disc` | `serial` (e.g. `SCUS_971.99`), `region` (e.g. `NTSC-U`), `version` (e.g. `1.00`), `supported` (bool) |
| `error` | `code`, `message` |
| `done` | `elapsed_ms` |

The process exit code equals the error code:

| Code | Meaning | Launcher message |
|---|---|---|
| 0 | ok | |
| 10 | cannot open/read file | That file couldn't be read. |
| 11 | not an ISO9660 image | That file isn't a disc image. |
| 20 | not Ratchet & Clank (unknown disc) | This doesn't look like Ratchet & Clank. |
| 21 | Ratchet & Clank, unsupported build/region (only SCUS_971.99 v1.00 for now) | This copy isn't supported yet — only the US release (SCUS-97199) for now. |
| 30 | write failure | The game data couldn't be written. |
| 31 | not enough disk space | There isn't enough disk space. |
| 40 | verification failed | Some game files don't match. |
| 99 | internal error | Something went wrong inside the extractor. |

On success the extractor writes `<game_data_dir>/extract-info.json`:

```json
{"disc":"SCUS_971.99","data_format":1,"extractor_version":"...","ntsc_only":false,"files":0,"bytes":0}
```

`ntsc_only` says the PAL copies were skipped (`--ntsc-only`). Only R&C1's extractor has that option, and the
launcher passes it to no other game; a game that never skips anything may leave the field out, which means
`false`. Other keys are ignored.

## Runtime

- `<runtime> --version-json` prints `{"name":"rerac","version":"...","game":"rac1","data_format":1}`
  and exits.
- The launcher starts the game with `<runtime> --data-dir <game_data_dir>`. The env var
  `RC_DATA_DIR` is equivalent; the launcher sets both.
- Game side (clarification): `--version-json` answers in milliseconds without opening a window.
  `--data-dir <dir>` / `--data-dir=<dir>` wins over `RC_DATA_DIR`. The folder must hold `toc.bin`
  and an `extract-info.json` with the build's `data_format`.
- When the runtime cannot start, it prints one `error: ...` line on stderr and exits before any
  window opens (game repo clarification 14):

  | Exit | Raised when | Launcher |
  |---|---|---|
  | 0 | normal quit (also after `--version-json`) | |
  | 2 | `--data-dir` given without a value | "ReRAC couldn't start." (a launcher/game mismatch) |
  | 3 | the data folder is missing, is not a data folder (no `toc.bin`), or the extraction is incomplete (no `extract-info.json`) | "The game data is missing or incomplete." + Re-extract |
  | 4 | `extract-info.json` has another `data_format`, or cannot be read | "The game data doesn't match this ReRAC version." + Re-extract |

  The launcher shows the `error:` line under Details. Any other non-zero exit (a panic is 101; a
  signal has no code) is a crash; the launcher says so and points at the log. Unknown arguments
  are ignored with a warning, so a newer launcher's extra flags do not stop an older runtime.
- **Packaged version folder** (game repo clarification 15, `tools/package/package.sh`):
  `rerac` (`rerac.exe` on Windows), `rerac-extract`, `assets/shaders/*.wgsl`,
  `rerac-manifest.json`, `README.txt`. The manifest names the binaries by these relative paths.
  The runtime finds `assets/` next to its real (symlink-resolved) executable, so the folder can be
  copied or moved as a whole, but `rerac` must not be copied out of it alone. Releases are
  published as `rerac-<version>-<os>-<arch>.zip` holding that folder as its single top-level
  entry (e.g. `rerac-0.1.0-macos-arm64/`).

## Folders

Per-OS app data root named `rerac`:

| OS | Default |
|---|---|
| macOS | `~/Library/Application Support/rerac/` |
| Windows | `%LOCALAPPDATA%\rerac\` |
| Linux | `$XDG_DATA_HOME/rerac/` (default `~/.local/share/rerac/`) |

Under it:

```
versions/            game builds by source: versions/official/<tag>/, versions/development/<version>/
                     (installed zips), later versions/mods/...
games/rac1/data/     the extracted game data (the runtime's --data-dir)
logs/
settings/            settings/launcher.json holds the launcher settings
```

The user can move the data root. User-facing text says **ReRAC** (technical identifier `rerac`); no other project
name appears anywhere user-facing.

Launcher side only (not part of the game contract): before the rename the root was named `randcrw`. On start, when
the default `rerac` root is missing and a `randcrw` folder sits next to it, the launcher copies it once to `rerac`
(through a `rerac.migrating` staging folder), leaves the old folder untouched and notes the copy in
`logs/launcher.log`. With `RERAC_DATA_ROOT` set, nothing is copied.

## Clarifications (game side)

The canonical copy of the contract and of these clarifications lives in the game repo at
`docs/plan/launcher_contract.md`. None of them changes a field, a command or a code.

1. Every run ends with exactly one `done` line (exit 0) or one `error` line (exit = its code).
   The exit code is authoritative if the stream is cut short.
2. Objects may carry extra fields; the launcher ignores unknown ones. The `disc` line has
   `game` (`rac1`, `rac2`, `rac3`, `racdl` or `unknown`), `title` and `elf_sha1`.
3. The `disc` line appears whenever the boot ELF was read: on success, for code 21, and for code
   20 when the image is another PS2 game. Never for 10 or 11.
4. The sequels (R&C 2, 3, Deadlocked) return **21** with `game` set, not 20. The launcher says
   e.g. "That's Ratchet & Clank 2 — not supported yet."
5. A truncated image returns 10; a raw 2352-byte `.bin` returns 11. Usage errors return 99 with a
   message starting `usage:`.
6. `extract` reports the stages `identify` then `copy`; every file is hashed while it is copied
   and a mismatch returns 40. `verify` reports `verify`. Progress is rate-limited to about 10
   lines a second and each stage ends with `done == total`.
7. Cancel = kill. Files are written as `.partial` and renamed when complete;
   `extract-info.json` is deleted at start and written last, so a folder without it is
   incomplete and is never launched.
8. `--iso` falls back to `RC_ISO`; `--threads N`, `--help` and `--version` exist.
9. A full extraction of about 4 GiB takes 3–8 s, `verify` about 2 s.
10. **Exports (optional).** `rerac-extract export --out <game_data_dir> [--to <dir>] [--what
    textures,audio,models,levels,collision,text|all] [--level NN] [--json]` writes PNG / WAV / glTF /
    JSON from the installed data (never the disc). Progress uses the stage `export`; `done`/`total`
    are bytes of installed data read. `export-info.json` in `--to` is written last; files are
    `.partial` until complete. Codes: 10 (data unreadable / not installed), 30/31 (writing the
    export failed / disk full), 40 (installed data damaged: verify), 99. Items a loader rejects are
    `info` lines starting `skipped: `. The whole disc is about 60,000 files and 3.5 GB in about 10 s.

## Launcher-side details (not part of the game's obligations)

These are how the launcher uses the contract. The game side does not need to do anything for them.

- **Export assets…** (RAC1 game screen, secondary menu). A modal picks the destination (default
  `games/<game>/data/exports/`; a picked folder that is neither empty nor an earlier export gets a
  `rerac-<game>-exports/` subfolder) and the kinds, then runs `export --out games/<game>/data
  --to <dir> --what <kinds> --json` as a job like `verify` (same progress panel, cancel = kill,
  log `logs/export-<game>-<unix>.log`). It never touches `data/`. The result panel offers "Open
  folder"; failures use their own messages (no re-extract offer).

- **Staging.** The launcher runs `extract` with `--out games/<game>/data.staging/`, and renames
  it over `games/<game>/data/` only after exit code 0 and a readable `extract-info.json`. A failed
  or cancelled run deletes the staging folder and leaves an existing install untouched.
- **Cancel** kills the extractor process.
- **Throttling.** The launcher forwards at most about 12 progress events a second per stage to
  the page, plus every stage change and each stage's final line.
- **Outcome.** The exit code decides success or failure. The message shown comes from the last
  `error` line with the same code, else the last stderr line. Lines on stdout that are not
  contract objects are treated as `info`.
- **Moved data root.** When the user moves the data root, `location.json`
  (`{"data_root": "<absolute path>"}`) is written into the default root so the launcher finds it
  again. `RERAC_DATA_ROOT` overrides the data root for development.
- **Stale data.** If the installed `extract-info.json` has a different `data_format` than the
  active version's manifest, the launcher asks for a re-extract instead of starting the game.
- **Validation.** A version is usable when its manifest parses, both binaries exist, and
  `<runtime> --version-json` answers within 10 s with `name` = `rerac` and the same `game` and
  `data_format` as the manifest.
- **Installing a version** (`src-tauri/src/install.rs`). "Install from zip…" (Development) and
  Official downloads take the same path: unpack into a hidden `versions/<source>/.install-*`
  folder (skipping `__MACOSX/` and AppleDouble `._*` entries, keeping the zip's Unix permission
  bits, allowing only relative symlinks that stay inside), use the folder that holds the manifest
  (the zip root or its single top-level folder), validate as above, then rename it to
  `versions/<source>/<id>/`. The id is the manifest's `version` for a local zip and the release tag
  for an Official download. Nothing is left behind on failure. Installed versions can be deleted
  from Version Management; registered development folders are only taken off the list.
- **macOS quarantine.** The launcher removes `com.apple.quarantine` from installed versions and
  from the two binaries of a registered development folder. A zip downloaded with a browser is
  quarantined, Finder's Archive Utility copies the flag onto every unpacked file, and Gatekeeper
  then refuses to run the ad-hoc-signed `rerac` ("developer cannot be verified").
- **Active version.** The extractor for Install/Verify and the runtime for Play both come from the
  active version. Switching to a version with another `data_format` than the installed data warns
  right away, and Play is replaced by a Re-extract prompt.
- **Play.** `<version>/rerac --data-dir <root>/games/<game>/data` (plus `RC_DATA_DIR`), working
  directory the version folder. Optionally the launcher minimises itself while the game runs
  (setting `minimize_while_playing`, default off) and comes back when it exits.
- **Logs.** `logs/extract-<game>-<unix>.log`, `logs/verify-<game>-<unix>.log` and
  `logs/<game>-<unix>.log` (game stdout and stderr, with the command line first and the exit code
  last; a second start in the same second gets `-2`, `-3`, …). A failed game run offers "Open logs",
  which reveals that file.
