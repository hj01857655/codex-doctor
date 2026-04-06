# codex-doctor

[![CI](https://github.com/hj01857655/codex-doctor/actions/workflows/ci.yml/badge.svg)](https://github.com/hj01857655/codex-doctor/actions/workflows/ci.yml)

A cross-platform CLI and GUI tool for diagnosing and repairing local Codex state issues.

## Current Status

- **What is covered today**:
  - CLI: `scan`, `diagnose`, `repair` (dry-run/main plus `--save-history`), `history`, `backup list/restore/prune` in both JSON and human-readable modes.
  - GUI: Dashboard scan/preview/execute flows, Backups tab (list + restore), History tab (list + detail), and guards for empty/no-selection states.
  - Core: repair history persistence, backup manifest snapshots, and extended test coverage across repair/diagnosis/backup/history pipelines.
- **Verification**: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test` all pass on current tree.

## What is codex-doctor?

`codex-doctor` helps you fix common problems with your local `.codex` directory:

- Sessions not visible after switching providers
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

```bash
cargo build --release
```

Binaries will be available at:
- CLI: `target/release/cli` (or `cli.exe` on Windows)
- GUI: `target/release/gui` (or `gui.exe` on Windows)

## CLI Usage

### Scan your Codex home

```bash
codex-doctor scan --codex-home ~/.codex
```

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

With an explicit SQLite home override:
```bash
codex-doctor diagnose --codex-home ~/.codex --sqlite-home ~/.codex-sqlite --json
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

## Safety Model

- **Automatic backups**: Every repair operation creates a timestamped backup
- **Dry-run first**: Preview changes before applying them
- **Atomic operations**: Repairs are applied in order with rollback support
- **Lock detection**: Skips locked files/databases and reports them as retryable
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
- Backups include: config.toml, sessions/, archived_sessions/, state_5.sqlite, logs_2.sqlite (if present), history.jsonl (if present)

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

## License

See LICENSE file for details.

## Contributing

This is a community tool for local state repair. Contributions welcome via pull requests.
