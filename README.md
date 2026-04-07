# codex-doctor

[![CI](https://github.com/hj01857655/codex-doctor/actions/workflows/ci.yml/badge.svg)](https://github.com/hj01857655/codex-doctor/actions/workflows/ci.yml)

A cross-platform CLI and GUI tool for diagnosing and repairing local Codex state issues.

`codex-doctor` is built to fix Codex local session-state chain mismatches — the class of problems where history still exists on disk, but sessions become invisible, indexes drift out of sync, metadata conflicts appear, and manual repair is risky.

## Current Status

- **What is covered today**:
- CLI: `scan`, `diagnose`, `repair` (dry-run/main plus `--save-history`), `history`, `backup list/restore/prune` in both JSON and human-readable modes.
  - Plus `resume-doctor` for explaining why default `codex resume` / `/resume` cannot see a session and for surfacing direct recovery commands.
  - GUI: Dashboard scan/preview/execute flows, Backups tab (list + restore), History tab (list + detail), and guards for empty/no-selection states.
  - Core: repair history persistence, backup manifest snapshots, `history.jsonl` / `logs_1.sqlite` state visibility, and extended test coverage across repair/diagnosis/backup/history pipelines.
- **Verification**: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test` all pass on current tree.

## What is codex-doctor?

`codex-doctor` helps you fix common problems with your local `.codex` directory:

- Sessions not visible after switching providers
- Sessions present on disk but hidden from the default resume picker because the current `model_provider` changed
- Archived sessions that still exist but will not show up in the default resume picker
- Mismatched data between `sessions/`, `archived_sessions/`, and SQLite
- Stale rollout paths in the SQLite index
- Missing or inconsistent configuration
- Provider metadata conflicts

## What it does NOT do

- Does not handle authentication or account switching
- Does not modify message content, timestamps, or titles
- Does not proxy or modify upstream API requests
- Does not depend on proprietary Codex IDE internals

## Installation

### Recommended: download prebuilt binaries from Releases

For normal users, the recommended path is to download a prebuilt release from GitHub Releases instead of building from source.

The repository ships a tag-driven release workflow at `.github/workflows/release.yml`. Pushing a tag that matches `v*` builds and uploads these release assets:
- `codex-doctor-windows-x86_64.zip`
- `codex-doctor-linux-x86_64.tar.gz`
- `codex-doctor-macos-x86_64.tar.gz`

Each archive contains:
- `codex-doctor` / `codex-doctor.exe`
- `gui` / `gui.exe`
- `README.md`

After extracting:

```bash
codex-doctor scan --codex-home ~/.codex
```

On Windows:

```powershell
.\codex-doctor.exe scan --codex-home C:\Users\<you>\.codex
```

### Build from source

```bash
cargo build --release
```

Binaries will be available at:
- CLI: `target/release/codex-doctor` (or `codex-doctor.exe` on Windows)
- GUI: `target/release/gui` (or `gui.exe` on Windows)

If you already have Rust installed, you can install the CLI as a normal command:

```bash
cargo install --path apps/cli --bin codex-doctor --force
```

After that, you can run:

```bash
codex-doctor scan --codex-home ~/.codex
```

## CLI Usage

### Scan your Codex home

```bash
codex-doctor scan --codex-home ~/.codex
```

The scan summary reports whether these local state surfaces are present/readable:
- `config.toml`
- `state_5.sqlite`
- `logs_1.sqlite`
- `history.jsonl`
- `sessions/` availability
- locked database / locked rollout state

With JSON output:
```bash
codex-doctor scan --codex-home ~/.codex --json
```

With an explicit SQLite home override:
```bash
codex-doctor scan --codex-home ~/.codex --sqlite-home ~/.codex-sqlite --json
```

### Diagnose problems

```bash
codex-doctor diagnose --codex-home ~/.codex
```

`diagnose` now also distinguishes between:
- sessions that are actually broken because local state drifted, and
- sessions that still exist but are likely hidden from default `codex resume` / `/resume` flows by provider or archived filtering.

With an explicit SQLite home override:
```bash
codex-doctor diagnose --codex-home ~/.codex --sqlite-home ~/.codex-sqlite --json
```

### Explain why `/resume` is empty

```bash
codex-doctor resume-doctor
```

This command reports:
- current-cwd local sessions by default,
- whether they are visible to the default `/resume` picker,
- which blocker applies (`provider mismatch`, `cwd mismatch`, `archived`, `missing sqlite row`),
- and the direct recovery command when possible.

By default it assumes:
- `codex_home = ~/.codex` (or `%USERPROFILE%\\.codex` on Windows)
- `current_cwd = your current shell directory`

By default, `resume-doctor` only shows sessions whose stored `cwd` matches your current shell directory, and it sorts them newest first to match the expected `/resume` workflow more closely.

If you want to simulate opening Codex from a specific directory:

```bash
codex-doctor resume-doctor --current-cwd /path/to/project --json
```

Only pass `--codex-home` when your Codex state is not in the default home location.

If you want to inspect sessions from other directories too:

```bash
codex-doctor resume-doctor --all
```

### Preview repair plan (dry-run)

```bash
codex-doctor repair --codex-home ~/.codex --backups-root ~/.codex-backups --dry-run
```

### Execute repair

```bash
codex-doctor repair --codex-home ~/.codex --backups-root ~/.codex-backups
```

**Important:** A backup is automatically created before any repair operation.

If your SQLite state lives outside the main `.codex` directory, pass it explicitly:

```bash
codex-doctor repair --codex-home ~/.codex --sqlite-home ~/.codex-sqlite --backups-root ~/.codex-backups
```

To also persist a structured repair history entry:

```bash
codex-doctor repair --codex-home ~/.codex --backups-root ~/.codex-backups --save-history
```

Repair history is stored under:

```text
<codex-home>/.codex-doctor/history
```

List saved repair history as JSON:

```bash
codex-doctor history --history-dir ~/.codex/.codex-doctor/history --json
```

List saved repair history in human-readable form:

```bash
codex-doctor history --history-dir ~/.codex/.codex-doctor/history
```

### Backup management

List backups:
```bash
codex-doctor backup list --backups-root ~/.codex-backups
```

Restore from backup:
```bash
codex-doctor backup restore --snapshot-dir ~/.codex-backups/backup-20260406-123456 --codex-home ~/.codex
```

Prune old backups (keep latest N):
```bash
codex-doctor backup prune --backups-root ~/.codex-backups --keep-latest 5
```

## GUI Usage

Launch the GUI:
```bash
gui ~/.codex
```

Or launch without arguments and enter the path in the UI:
```bash
gui
```

The GUI also supports an explicit `SQLite home` field when the state database is stored outside the main Codex home.

The GUI provides:
- Visual summary of your Codex state
- Problem list with severity indicators
- Repair plan preview
- One-click repair execution
- Backup management
- Repair history browsing

The GUI navigation currently includes:
- `Dashboard` for scan results, diagnosis, and repair preview
- `Backups` for listing and restoring backups
- `History` for browsing saved repair executions

When a repair is executed from the GUI, it writes:
- backups to `<codex-home>/.codex-doctor-backups`
- repair history to `<codex-home>/.codex-doctor/history`

## Supported Repair Types

| Problem | Repair Action |
|---------|---------------|
| Missing SQLite thread row | Rebuild from rollout metadata |
| Stale SQLite rollout path | Update path or rebuild index |
| Provider mismatch | Rewrite rollout and SQLite metadata |
| Archived state mismatch | Move rollout to correct location |
| Missing root model provider | Patch config.toml |
| Missing `sessions/` directory | Diagnose and report missing local session storage |
| Unreadable `state_5.sqlite` | Diagnose and report unreadable SQLite state |
| Locked database / rollout files | Detect lock conflicts and mark retryable execution paths |
| Missing / unreadable `history.jsonl` | Diagnose and report local history state |
| Missing / unreadable `logs_1.sqlite` | Diagnose and report logs database state |

## Safety Model

- **Automatic backups**: Every repair operation creates a timestamped backup
- **Dry-run first**: Preview changes before applying them
- **Atomic operations**: Repairs are applied in order with rollback support
- **Lock detection**: Skips locked files/databases and reports them as retryable
- **Retry guidance**: Repair output and history explicitly preserve whether a skipped item is retryable
- **Evidence-based**: Only repairs problems with clear evidence

## Platform Notes

### Windows
- SQLite databases may be locked by running Codex processes
- Close Codex before running repairs
- File locks are detected and reported as retryable

### macOS/Linux
- Same lock detection applies
- Ensure proper file permissions on `.codex` directory

### All Platforms
- Backup directory should be on the same filesystem for best performance
- Backups include: config.toml, sessions/, archived_sessions/, state_5.sqlite, logs_1.sqlite (if present), history.jsonl (if present)

## Architecture

```
codex-doctor/
├── apps/cli/          # CLI shell
├── apps/gui/          # GUI shell (egui)
└── crates/doctor-core # Shared core logic
```

Both CLI and GUI use the same core library, ensuring consistent behavior.

## Development

Run tests:
```bash
cargo test --workspace
```

Run with formatting check:
```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

## Known Limitations

- Does not repair corrupted SQLite databases (only metadata mismatches)
- Requires manual intervention for severely corrupted rollout files
- Cannot repair while Codex is actively using the database
- Lock/busy failures are reported with retry guidance, but file/database ownership still needs to be resolved manually

## License

See LICENSE file for details.

## Contributing

This is a community tool for local state repair. Contributions welcome via pull requests.
