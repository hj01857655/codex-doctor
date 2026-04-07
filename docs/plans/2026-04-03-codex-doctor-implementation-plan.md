# codex-doctor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a cross-platform Codex local-state doctor that can scan, diagnose, back up, repair, and restore `.codex` data via shared Rust core, CLI, and GUI shells.

**Architecture:** Use a Rust workspace with one reusable core crate (`doctor-core`) that owns layout discovery, scan, diagnosis, backup, repair planning, and execution. Expose the same core data contracts to both a CLI app and a GUI app so scan/repair behavior stays identical across surfaces.

**Tech Stack:** Rust workspace, `clap` for CLI, `serde`/`serde_json`, `toml`, `rusqlite` or `sqlx` (pick one and stay consistent), `tauri` or `egui` for GUI shell, fixture-driven integration tests.

---

### Task 1: Scaffold the Rust workspace

**Files:**
- Create: `e:\VSCodeSpace\codex-doctor\Cargo.toml`
- Create: `e:\VSCodeSpace\codex-doctor\rustfmt.toml`
- Create: `e:\VSCodeSpace\codex-doctor\apps\cli\Cargo.toml`
- Create: `e:\VSCodeSpace\codex-doctor\apps\cli\src\main.rs`
- Create: `e:\VSCodeSpace\codex-doctor\apps\gui\Cargo.toml`
- Create: `e:\VSCodeSpace\codex-doctor\apps\gui\src\main.rs`
- Create: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\Cargo.toml`
- Create: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\src\lib.rs`

**Step 1: Write the failing compile target**

Create a workspace manifest that lists `apps/cli`, `apps/gui`, and `crates/doctor-core`, but do not create all source files yet.

**Step 2: Run build to verify it fails**

Run:
```powershell
cd e:\VSCodeSpace\codex-doctor
cargo check
```
Expected: FAIL because one or more member crates or source files are missing.

**Step 3: Write minimal implementation**

Create the three crates with minimal compile-safe code:

```rust
fn main() {
    println!("codex-doctor cli bootstrap");
}
```

```rust
fn main() {
    println!("codex-doctor gui bootstrap");
}
```

```rust
pub fn version_banner() -> &'static str {
    "codex-doctor core"
}
```

**Step 4: Run build to verify it passes**

Run:
```powershell
cargo check
```
Expected: PASS.

**Step 5: Commit**

```powershell
git add Cargo.toml rustfmt.toml apps crates
git commit -m "chore: scaffold codex-doctor workspace"
```

### Task 2: Model `.codex` layout discovery

**Files:**
- Create: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\src\layout.rs`
- Modify: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\src\lib.rs`
- Create: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\tests\layout_tests.rs`
- Use for reference only: `e:\VSCodeSpace\codex-doctor\.tmp\codex\docs\config.md`
- Use for reference only: `e:\VSCodeSpace\codex-doctor\.tmp\codex\codex-rs\state\src\lib.rs`
- Use for reference only: `e:\VSCodeSpace\codex-doctor\.tmp\codex\codex-rs\state\src\runtime.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn resolves_default_layout_from_codex_home() {
    let layout = CodexLayout::from_codex_home("/tmp/example/.codex");
    assert!(layout.config_toml.ends_with("config.toml"));
    assert!(layout.sessions_dir.ends_with("sessions"));
    assert!(layout.archived_sessions_dir.ends_with("archived_sessions"));
    assert!(layout.state_db.ends_with("state_5.sqlite"));
}
```

**Step 2: Run test to verify it fails**

Run:
```powershell
cargo test -p doctor-core layout_tests -- --nocapture
```
Expected: FAIL because `CodexLayout` does not exist.

**Step 3: Write minimal implementation**

Implement a layout struct with fields for:
- `codex_home`
- `config_toml`
- `sessions_dir`
- `archived_sessions_dir`
- `state_db`
- `logs_db`
- `history_jsonl`
- `sqlite_home`

Bake in the confirmed defaults from the Codex source analysis:
- state DB filename = `state_5.sqlite`
- logs DB filename = `logs_1.sqlite`
- `CODEX_SQLITE_HOME` overrides state DB home when explicitly provided.

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p doctor-core layout_tests -- --nocapture
```
Expected: PASS.

**Step 5: Commit**

```powershell
git add crates/doctor-core/src/layout.rs crates/doctor-core/src/lib.rs crates/doctor-core/tests/layout_tests.rs
git commit -m "feat: add codex layout discovery"
```

### Task 3: Add rollout and config domain models

**Files:**
- Create: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\src\model.rs`
- Create: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\src\rollout.rs`
- Create: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\src\config.rs`
- Modify: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\src\lib.rs`
- Create: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\tests\rollout_model_tests.rs`
- Use for reference only: `e:\VSCodeSpace\codex-doctor\.tmp\codex\codex-rs\rollout\src\metadata.rs`

**Step 1: Write the failing tests**

Create tests that assert:
- a rollout file can report `thread_id`
- `session_meta.model_provider` is extracted when present
- root `model_provider` can be read from `config.toml`
- archived vs active location is represented in a domain enum

**Step 2: Run tests to verify they fail**

Run:
```powershell
cargo test -p doctor-core rollout_model_tests -- --nocapture
```
Expected: FAIL because parsers and types do not exist.

**Step 3: Write minimal implementation**

Implement:
- `RolloutRecord`
- `RolloutSessionMeta`
- `ThreadLocation::{Active, Archived}`
- `RootConfigSnapshot`

Support only the fields needed for first-pass diagnosis:
- thread id
- rollout path
- provider
- cwd
- timestamp
- archived flag

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p doctor-core rollout_model_tests -- --nocapture
```
Expected: PASS.

**Step 5: Commit**

```powershell
git add crates/doctor-core/src/model.rs crates/doctor-core/src/rollout.rs crates/doctor-core/src/config.rs crates/doctor-core/tests/rollout_model_tests.rs
git commit -m "feat: add rollout and config models"
```

### Task 4: Add SQLite thread metadata reader

**Files:**
- Create: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\src\sqlite.rs`
- Modify: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\src\lib.rs`
- Create: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\tests\sqlite_reader_tests.rs`
- Use for reference only: `e:\VSCodeSpace\codex-doctor\.tmp\codex\codex-rs\state\src\model\thread_metadata.rs`

**Step 1: Write the failing test**

Create a temp SQLite database with a `threads` table row and assert the reader can load:
- `id`
- `rollout_path`
- `model_provider`
- `archived_at`
- `cwd`

**Step 2: Run test to verify it fails**

Run:
```powershell
cargo test -p doctor-core sqlite_reader_tests -- --nocapture
```
Expected: FAIL because the SQLite adapter does not exist.

**Step 3: Write minimal implementation**

Implement a read-only SQLite adapter with:
- `read_threads()`
- `read_thread_by_id()`
- structured error types for open failure vs query failure

Do not write to SQLite yet.

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p doctor-core sqlite_reader_tests -- --nocapture
```
Expected: PASS.

**Step 5: Commit**

```powershell
git add crates/doctor-core/src/sqlite.rs crates/doctor-core/tests/sqlite_reader_tests.rs
git commit -m "feat: add sqlite thread metadata reader"
```

### Task 5: Build the scan pipeline

**Files:**
- Create: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\src\scan.rs`
- Modify: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\src\lib.rs`
- Create: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\tests\scan_tests.rs`
- Create fixtures under: `e:\VSCodeSpace\codex-doctor\tests\fixtures\scan\...`

**Step 1: Write the failing integration test**

Create a fixture `.codex` tree with:
- one active rollout
- one archived rollout
- one `config.toml`
- one SQLite thread row

Assert `scan_codex_home()` returns summary counts and provider distribution.

**Step 2: Run test to verify it fails**

Run:
```powershell
cargo test -p doctor-core scan_tests -- --nocapture
```
Expected: FAIL because the scanner does not exist.

**Step 3: Write minimal implementation**

Implement scanner output structs:
- `ScanSummary`
- `ProviderDistribution`
- `ScanReport`

Include:
- file presence
- active count
- archived count
- sqlite readable flag
- root provider from config
- provider counts from rollout and sqlite

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p doctor-core scan_tests -- --nocapture
```
Expected: PASS.

**Step 5: Commit**

```powershell
git add crates/doctor-core/src/scan.rs crates/doctor-core/tests/scan_tests.rs tests/fixtures/scan
git commit -m "feat: add codex scan pipeline"
```

### Task 6: Build diagnosis rules for first-version failures

**Files:**
- Create: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\src\diagnose.rs`
- Modify: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\src\lib.rs`
- Create: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\tests\diagnose_tests.rs`

**Step 1: Write the failing tests**

Add one test per diagnosis type:
- `MissingSqliteThreadRow`
- `StaleSqliteRolloutPath`
- `RolloutProviderMismatch`
- `ArchivedStateMismatch`
- `MissingRootModelProvider`

Example:

```rust
#[test]
fn flags_stale_sqlite_rollout_path() {
    let problems = diagnose(report_with_missing_rollout_for_sqlite_path());
    assert!(problems.iter().any(|p| p.code == "stale_sqlite_rollout_path"));
}
```

**Step 2: Run tests to verify they fail**

Run:
```powershell
cargo test -p doctor-core diagnose_tests -- --nocapture
```
Expected: FAIL because diagnosis rules do not exist.

**Step 3: Write minimal implementation**

Implement:
- `ProblemSeverity`
- `ProblemCode`
- `DiagnosisReport`
- `diagnose(scan_report)`

Each problem should include:
- code
- severity
- evidence
- suggested fix ids

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p doctor-core diagnose_tests -- --nocapture
```
Expected: PASS.

**Step 5: Commit**

```powershell
git add crates/doctor-core/src/diagnose.rs crates/doctor-core/tests/diagnose_tests.rs
git commit -m "feat: add diagnosis rules"
```

### Task 7: Add backup snapshot creation and retention

**Files:**
- Create: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\src\backup.rs`
- Modify: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\src\lib.rs`
- Create: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\tests\backup_tests.rs`
- Create fixtures under: `e:\VSCodeSpace\codex-doctor\tests\fixtures\backup\...`

**Step 1: Write the failing tests**

Assert that backup creation copies:
- `config.toml`
- `sessions/`
- `archived_sessions/`
- `state_5.sqlite`
- optional `logs_1.sqlite`
- optional `history.jsonl`

Also assert prune keeps only the newest N backups.

**Step 2: Run tests to verify they fail**

Run:
```powershell
cargo test -p doctor-core backup_tests -- --nocapture
```
Expected: FAIL because backup logic does not exist.

**Step 3: Write minimal implementation**

Implement:
- `create_backup_snapshot()`
- `list_backups()`
- `restore_backup()`
- `prune_backups()`

Store backup metadata in a small manifest JSON file inside each backup directory.

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p doctor-core backup_tests -- --nocapture
```
Expected: PASS.

**Step 5: Commit**

```powershell
git add crates/doctor-core/src/backup.rs crates/doctor-core/tests/backup_tests.rs tests/fixtures/backup
git commit -m "feat: add backup and restore support"
```

### Task 8: Add repair planning contracts

**Files:**
- Create: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\src\plan.rs`
- Modify: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\src\lib.rs`
- Create: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\tests\plan_tests.rs`

**Step 1: Write the failing tests**

Create tests that map diagnosis results to actions:
- stale rollout path -> `RebuildMissingIndexFromRollout`
- missing sqlite row -> `UpsertSqliteThreadMetadata`
- archived mismatch -> `MoveRolloutToArchive` or `MoveRolloutToSessions`
- provider mismatch -> `RewriteRolloutSessionMeta` plus `UpsertSqliteThreadMetadata`
- missing root provider -> `PatchConfigModelProvider`

**Step 2: Run tests to verify they fail**

Run:
```powershell
cargo test -p doctor-core plan_tests -- --nocapture
```
Expected: FAIL because repair planning types do not exist.

**Step 3: Write minimal implementation**

Implement:
- `RepairAction`
- `RepairPlan`
- `build_repair_plan()`
- dry-run summary rendering

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p doctor-core plan_tests -- --nocapture
```
Expected: PASS.

**Step 5: Commit**

```powershell
git add crates/doctor-core/src/plan.rs crates/doctor-core/tests/plan_tests.rs
git commit -m "feat: add repair plan builder"
```

### Task 9: Implement repair execution

**Files:**
- Create: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\src\repair.rs`
- Modify: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\src\sqlite.rs`
- Modify: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\src\rollout.rs`
- Modify: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\src\config.rs`
- Modify: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\src\backup.rs`
- Create: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\tests\repair_tests.rs`

**Step 1: Write the failing integration tests**

Add repair tests for:
- upserting a missing SQLite thread row from rollout session metadata
- rewriting rollout provider metadata
- moving archived rollout back to sessions
- patching missing root provider in config
- dry-run causing zero writes

**Step 2: Run tests to verify they fail**

Run:
```powershell
cargo test -p doctor-core repair_tests -- --nocapture
```
Expected: FAIL because write-path logic does not exist.

**Step 3: Write minimal implementation**

Implement execution flow:
1. create backup
2. execute actions in order
3. collect `applied`, `skipped`, `failed`
4. return machine-readable execution report

Ensure locked files or busy databases become `skipped` or `failed` with retryable metadata instead of panicking.

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p doctor-core repair_tests -- --nocapture
```
Expected: PASS.

**Step 5: Commit**

```powershell
git add crates/doctor-core/src/repair.rs crates/doctor-core/src/sqlite.rs crates/doctor-core/src/rollout.rs crates/doctor-core/src/config.rs crates/doctor-core/src/backup.rs crates/doctor-core/tests/repair_tests.rs
git commit -m "feat: implement repair execution"
```

### Task 10: Wire the CLI commands

**Files:**
- Modify: `e:\VSCodeSpace\codex-doctor\apps\cli\src\main.rs`
- Create: `e:\VSCodeSpace\codex-doctor\apps\cli\tests\cli_smoke.rs`

**Step 1: Write the failing smoke tests**

Add smoke tests for:
- `codex-doctor scan --json`
- `codex-doctor diagnose --json`
- `codex-doctor repair --dry-run --json`
- `codex-doctor backup list --json`

**Step 2: Run tests to verify they fail**

Run:
```powershell
cargo test -p cli --test cli_smoke -- --nocapture
```
Expected: FAIL because the CLI only prints a placeholder string.

**Step 3: Write minimal implementation**

Implement subcommands with `clap`:
- `scan`
- `diagnose`
- `repair`
- `backup list`
- `backup restore`
- `backup prune`

Return non-zero exit codes on execution failure.

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p cli --test cli_smoke -- --nocapture
```
Expected: PASS.

**Step 5: Commit**

```powershell
git add apps/cli/src/main.rs apps/cli/tests/cli_smoke.rs
git commit -m "feat: wire codex-doctor cli"
```

### Task 11: Build the first GUI shell over the same core

**Files:**
- Modify: `e:\VSCodeSpace\codex-doctor\apps\gui\Cargo.toml`
- Modify: `e:\VSCodeSpace\codex-doctor\apps\gui\src\main.rs`
- Create additional GUI files as required by chosen framework
- Create: `e:\VSCodeSpace\codex-doctor\apps\gui\tests\gui_smoke.rs`

**Step 1: Write the failing GUI smoke test**

Add a smoke test that asserts the GUI layer can call the core scan function and render a summary view model.

**Step 2: Run test to verify it fails**

Run:
```powershell
cargo test -p gui gui_smoke -- --nocapture
```
Expected: FAIL because the GUI app is only a placeholder.

**Step 3: Write minimal implementation**

Create a minimal window with:
- codex home input
- refresh button
- summary panel
- problems list
- preview repair button
- execute repair button

The GUI should call the same core APIs used by the CLI, not spawn the CLI as a subprocess.

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p gui gui_smoke -- --nocapture
```
Expected: PASS.

**Step 5: Commit**

```powershell
git add apps/gui
git commit -m "feat: add first gui shell"
```

### Task 12: Add end-to-end fixtures for real repair scenarios

**Files:**
- Create fixtures under: `e:\VSCodeSpace\codex-doctor\tests\fixtures\e2e\...`
- Create: `e:\VSCodeSpace\codex-doctor\crates\doctor-core\tests\e2e_repair_tests.rs`
- Optionally modify any core modules required to support stable fixture-driven tests

**Step 1: Write the failing E2E tests**

Add fixture-backed tests for:
- provider mismatch across rollout + sqlite
- stale SQLite rollout path with file fallback
- archived thread recovery
- missing SQLite row repair
- config patch with backup creation

**Step 2: Run tests to verify they fail**

Run:
```powershell
cargo test -p doctor-core e2e_repair_tests -- --nocapture
```
Expected: FAIL because one or more real scenarios are not fully implemented.

**Step 3: Write minimal implementation**

Close any gaps needed for the E2E scenarios while keeping the implementation minimal.

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p doctor-core e2e_repair_tests -- --nocapture
```
Expected: PASS.

**Step 5: Commit**

```powershell
git add crates/doctor-core/tests/e2e_repair_tests.rs tests/fixtures/e2e
git commit -m "test: add end-to-end repair fixtures"
```

### Task 13: Write the user-facing project documentation

**Files:**
- Create: `e:\VSCodeSpace\codex-doctor\README.md`
- Create: `e:\VSCodeSpace\codex-doctor\docs\compatibility\2026-04-03-platform-and-locking-notes.md`
- Modify if needed: `e:\VSCodeSpace\codex-doctor\docs\design\2026-04-03-codex-doctor-design.md`

**Step 1: Write the failing docs checklist**

Create a short checklist in the task branch describing the minimum docs coverage:
- problem statement
- supported platforms
- supported repair types
- safety model
- backup / restore usage
- CLI examples
- GUI usage
- known limitations

**Step 2: Verify docs are incomplete**

Run a manual review of the repo root and confirm the README does not yet exist.
Expected: INCOMPLETE.

**Step 3: Write minimal implementation**

Document:
- what `codex-doctor` is
- what it changes and does not change
- CLI quickstart
- GUI quickstart
- backup / restore expectations
- platform caveats for locked files and busy SQLite DBs

**Step 4: Verify docs are complete**

Manual check: README and compatibility note both exist and match implemented behavior.
Expected: COMPLETE.

**Step 5: Commit**

```powershell
git add README.md docs/compatibility docs/design
git commit -m "docs: add user documentation"
```

### Task 14: Final verification pass

**Files:**
- Modify any file necessary to fix verification failures

**Step 1: Run full verification**

Run:
```powershell
cd e:\VSCodeSpace\codex-doctor
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --nocapture
```

**Step 2: Fix any failures**

Make only the minimal edits required.

**Step 3: Re-run verification**

Run the same commands again.
Expected: PASS.

**Step 4: Capture release-ready summary**

Record:
- implemented commands
- implemented repair types
- test coverage shape
- known follow-up items

**Step 5: Commit**

```powershell
git add .
git commit -m "chore: finalize codex-doctor v1 foundation"
```
