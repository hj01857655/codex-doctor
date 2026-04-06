# codex-doctor History and UX Closure Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Finish the current history, backup UX, and CLI/GUI polish branch by adding the missing regression coverage, documentation updates, and final verification around the already-landed repair-history and human-readable output flows.

**Architecture:** Keep `doctor-core` as the only place that owns repair-history persistence and backup metadata loading. Use CLI and GUI tests to lock the behavior at the boundary instead of adding new business logic branches, then update README to match the actually shipped commands and UI tabs.

**Tech Stack:** Rust workspace, `doctor-core`, `clap`, `serde_json`, `eframe/egui`, fixture-driven integration tests, PowerShell on Windows.

---

## Current validated baseline

Before starting tomorrow, assume the following is already true and should not be re-designed:

- `cargo test` passes on the current working tree.
- CLI already contains `repair --save-history` and `history --history-dir ...` entrypoints.
- GUI already contains Backups and History tabs.
- Core already contains `save_repair_history()` and `list_repair_history()`.

Tomorrow's work is therefore **closure and confidence**, not a new feature spike.

### Task 1: Lock CLI history persistence with failing smoke tests

**Files:**
- Modify: `apps/cli/tests/cli_smoke.rs`
- Verify only: `apps/cli/src/main.rs:38-68`
- Verify only: `apps/cli/src/main.rs:145-206`
- Verify only: `apps/cli/src/output.rs:144-173`

**Step 1: Write the failing tests**

Add two smoke tests to `apps/cli/tests/cli_smoke.rs`:

1. `repair_with_save_history_writes_history_entry_json`
2. `history_json_outputs_saved_entries`

Test shape:

```rust
#[test]
fn repair_with_save_history_writes_history_entry_json() {
    let codex_home = prepare_codex_home();
    let backups_root = tempdir().expect("create backups root");
    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");

    let output = Command::new(env!("CARGO_BIN_EXE_codex-doctor"))
        .args([
            "repair",
            "--codex-home",
            codex_home.path().to_str().expect("codex home path"),
            "--backups-root",
            backups_root.path().to_str().expect("backups root path"),
            "--save-history",
            "--json",
        ])
        .output()
        .expect("run repair cli");

    assert!(output.status.success());

    let history_dir = codex_home.path().join(".codex-doctor").join("history");
    let entries: Vec<_> = fs::read_dir(&history_dir).expect("read history dir").collect();
    assert_eq!(entries.len(), 1);
}
```

```rust
#[test]
fn history_json_outputs_saved_entries() {
    // first run repair --save-history
    // then run: codex-doctor history --history-dir <dir> --json
    // assert array len == 1 and codex_home matches fixture path
}
```

**Step 2: Run tests to verify they fail**

Run:
```powershell
cargo test -p cli --test cli_smoke -- --nocapture
```
Expected: FAIL because the new smoke cases do not exist yet.

**Step 3: Write minimal implementation**

Only if the tests expose real behavior gaps, make the minimum fix in:
- `apps/cli/src/main.rs`
- `apps/cli/src/output.rs`

Do **not** redesign command shape. Preserve:
- `repair --save-history`
- `history --history-dir ...`

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p cli cli_smoke -- --nocapture
```
Expected: PASS with both new smoke tests green.

**Step 5: Commit**

```powershell
git add apps/cli/tests/cli_smoke.rs apps/cli/src/main.rs apps/cli/src/output.rs
git commit -m "test: cover cli repair history flows"
```

### Task 2: Lock GUI Backups and History tabs with view-model regression tests

**Files:**
- Modify: `apps/gui/tests/gui_smoke.rs`
- Verify only: `apps/gui/src/lib.rs:64-66`
- Verify only: `apps/gui/src/lib.rs:122-139`
- Verify only: `apps/gui/src/lib.rs:318-388`

**Step 1: Write the failing tests**

Add two GUI smoke tests:

1. `load_backups_populates_backup_selection_state`
2. `load_history_reads_saved_repair_entries`

Test shape:

```rust
#[test]
fn load_backups_populates_backup_selection_state() {
    let codex_home = prepare_codex_home();
    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");

    let mut app = CodexDoctorApp::new(codex_home.path().display().to_string());
    app.execute_repair().expect("execute repair");
    app.load_backups().expect("load backups");

    assert!(!app.backups.is_empty());
    assert_eq!(app.selected_backup, None);
}
```

```rust
#[test]
fn load_history_reads_saved_repair_entries() {
    let codex_home = prepare_codex_home();
    fs::write(codex_home.path().join("config.toml"), "").expect("clear config");

    let mut app = CodexDoctorApp::new(codex_home.path().display().to_string());
    app.execute_repair().expect("execute repair");
    app.load_history().expect("load history");

    assert!(!app.history.is_empty());
    assert_eq!(app.selected_history, None);
    assert!(app.history[0].actions_applied >= 1);
}
```

If `execute_repair()` does not currently persist history, capture that as the failure and then fix the minimal missing call.

**Step 2: Run tests to verify they fail**

Run:
```powershell
cargo test -p gui gui_smoke -- --nocapture
```
Expected: FAIL because the new GUI smoke cases do not exist yet, or because execute/load glue is incomplete.

**Step 3: Write minimal implementation**

If required, patch only the minimum GUI glue in `apps/gui/src/lib.rs` so that:
- repair execution leaves history in the same on-disk location the CLI uses
- `load_backups()` and `load_history()` refresh state predictably
- selection indices reset to `None` after reload

Do not add rendering-only changes unless a test proves they are needed.

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p gui gui_smoke -- --nocapture
```
Expected: PASS with the new backup/history tests green.

**Step 5: Commit**

```powershell
git add apps/gui/tests/gui_smoke.rs apps/gui/src/lib.rs
git commit -m "test: cover gui backup and history flows"
```

### Task 3: Strengthen core history tests around action records and backup IDs

**Files:**
- Modify: `crates/doctor-core/tests/history_tests.rs`
- Verify only: `crates/doctor-core/src/history.rs:35-109`

**Step 1: Write the failing tests**

Add focused tests for:

1. `save_repair_history_persists_backup_id_when_present`
2. `save_repair_history_persists_action_statuses`

Use a synthetic `RepairExecutionReport` with at least one applied, skipped, and failed entry so the saved JSON must contain:
- correct `backup_id`
- correct `actions_applied/skipped/failed` counts
- at least one action record for each `ActionStatus`

**Step 2: Run tests to verify they fail**

Run:
```powershell
cargo test -p doctor-core history_tests -- --nocapture
```
Expected: FAIL because the new assertions are not yet implemented in test code, or because persistence misses one field.

**Step 3: Write minimal implementation**

If behavior gaps appear, patch only:
- `crates/doctor-core/src/history.rs`

Do not change JSON shape unless the tests prove a missing field or incorrect mapping.

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p doctor-core history_tests -- --nocapture
```
Expected: PASS.

**Step 5: Commit**

```powershell
git add crates/doctor-core/tests/history_tests.rs crates/doctor-core/src/history.rs
git commit -m "test: strengthen repair history persistence coverage"
```

### Task 4: Update README so shipped CLI and GUI behavior match docs

**Files:**
- Modify: `README.md`
- Verify only: `apps/cli/src/main.rs`
- Verify only: `apps/gui/src/lib.rs`

**Step 1: Write the failing docs checklist**

Before editing `README.md`, make a checklist in your scratchpad and confirm the README currently misses or under-specifies:
- `repair --save-history`
- `history --history-dir ...`
- GUI Backups tab
- GUI History tab
- the default on-disk history location under `<codex-home>/.codex-doctor/history`

Expected: checklist is incomplete against current behavior.

**Step 2: Verify current docs gap**

Run:
```powershell
rg -n "save-history|Repair History|Backups|history --history-dir|.codex-doctor/history" README.md
```
Expected: one or more items missing.

**Step 3: Write minimal documentation update**

Update `README.md` to cover:
- `repair --save-history`
- `history --history-dir <path>` example
- where history files are stored
- GUI now offering Summary / Backups / History style navigation

Keep docs aligned with real command names. Do not document features not in code.

**Step 4: Verify docs are complete**

Run:
```powershell
rg -n "save-history|Repair History|Backups|history --history-dir|.codex-doctor/history" README.md
```
Expected: all intended strings are present.

**Step 5: Commit**

```powershell
git add README.md
git commit -m "docs: document history and backup workflows"
```

### Task 5: Final verification and handoff summary

**Files:**
- Modify any file only if verification finds a real regression

**Step 1: Run format and test verification**

Run:
```powershell
cargo fmt --all --check
cargo test
```
Expected: PASS.

**Step 2: Run lint verification if time permits**

Run:
```powershell
cargo clippy --workspace --all-targets -- -D warnings
```
Expected: PASS.

If `clippy` fails, only fix warnings directly caused by this history/UX closure work.

**Step 3: Capture the evidence block for handoff**

Record:
- changed files
- added tests
- exact verification commands run
- whether history persistence is covered in CLI, GUI, and core layers

**Step 4: Re-check git status**

Run:
```powershell
git status --short --branch
```
Expected: only intentional plan-delivery or implementation changes remain.

**Step 5: Commit final closure batch**

```powershell
git add README.md apps/cli/tests/cli_smoke.rs apps/gui/tests/gui_smoke.rs crates/doctor-core/tests/history_tests.rs apps/cli/src/main.rs apps/gui/src/lib.rs crates/doctor-core/src/history.rs
git commit -m "feat: close history and backup ux loop"
```
