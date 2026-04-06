---
name: Bug report
about: Report a reproducible bug in the CLI, GUI, or repair core
title: "[bug] "
labels: bug
assignees: ""
---

## Summary

Describe the bug in 1-3 sentences.

## Affected Area

- [ ] CLI
- [ ] GUI
- [ ] doctor-core
- [ ] Backup / restore
- [ ] History / repair tracking
- [ ] CI / workflow

## Environment

- OS:
- Rust version (`rustc --version`):
- Cargo version (`cargo --version`):
- App path / workspace path:

## Reproduction Steps

1.
2.
3.

## Expected Behavior

Describe what should have happened.

## Actual Behavior

Describe what actually happened.

## Evidence

Paste the smallest useful evidence block:

```text
CLI output / GUI status / error text / stack trace
```

## Verification Commands

If relevant, list what you ran locally:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test
```

## Notes

Anything else that narrows scope, such as fixture used, whether `.codex-doctor-backups` or `.codex-doctor/history` was involved, or whether the bug depends on locked files / SQLite state.
