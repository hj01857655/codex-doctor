# codex-doctor Release-ready Summary

## Scope snapshot

`codex-doctor` 当前已经形成一版可交付的本地状态修复基础能力，覆盖：

- 共享 Rust 核心：layout / scan / diagnose / backup / repair / restore / history
- CLI：扫描、诊断、修复、备份管理、修复历史查询
- GUI：Dashboard / Backups / History 三个主导航面板

## Implemented commands

### CLI

- `codex-doctor scan --codex-home <path>`
- `codex-doctor diagnose --codex-home <path>`
- `codex-doctor repair --codex-home <path> --backups-root <path>`
- `codex-doctor repair --codex-home <path> --backups-root <path> --dry-run`
- `codex-doctor repair --codex-home <path> --backups-root <path> --save-history`
- `codex-doctor backup list --backups-root <path>`
- `codex-doctor backup restore --snapshot-dir <path> --codex-home <path>`
- `codex-doctor backup prune --backups-root <path> --keep-latest <n>`
- `codex-doctor history --history-dir <path>`

### GUI

- Dashboard：扫描摘要、问题列表、修复预览、执行修复
- Backups：加载备份、选择备份、恢复备份、清理旧备份
- History：加载修复历史、查看修复详情

## Implemented repair types

当前已经落地的修复动作：

- `RebuildMissingIndexFromRollout`
- `UpsertSqliteThreadMetadata`
- `MoveRolloutToArchive`
- `MoveRolloutToSessions`
- `RewriteRolloutSessionMeta`
- `PatchConfigModelProvider`

当前已覆盖的问题类型：

- `MissingSessionsDirectory`
- `UnreadableSqliteDatabase`
- `LockedDatabase`
- `LockedRolloutFile`
- `MissingSqliteThreadRow`
- `StaleSqliteRolloutPath`
- `RolloutProviderMismatch`
- `ArchivedStateMismatch`
- `MissingRootModelProvider`
- `MissingLogsSqlite`
- `UnreadableLogsSqlite`
- `MissingHistoryJsonl`
- `UnreadableHistoryJsonl`

说明：

- `logs_2.sqlite` 与 `history.jsonl` 当前纳入扫描、诊断、备份与恢复
- 这两类问题当前只做诊断，不自动生成修复动作

## Test coverage shape

当前测试覆盖按层次分布如下：

- `crates/doctor-core/tests/layout_tests.rs`
  - 路径发现、`sqlite_home` override
- `crates/doctor-core/tests/rollout_model_tests.rs`
  - rollout/config 元数据提取
- `crates/doctor-core/tests/sqlite_reader_tests.rs`
  - SQLite 线程元数据读取
- `crates/doctor-core/tests/scan_tests.rs`
  - 扫描汇总与 provider 分布
- `crates/doctor-core/tests/diagnose_tests.rs`
  - 诊断问题分类，包括 logs/history
- `crates/doctor-core/tests/plan_tests.rs`
  - 问题到修复动作的映射
- `crates/doctor-core/tests/repair_tests.rs`
  - 单项修复动作写入
- `crates/doctor-core/tests/backup_tests.rs`
  - 备份、恢复、清理
- `crates/doctor-core/tests/history_tests.rs`
  - 修复历史持久化与读取
- `crates/doctor-core/tests/e2e_repair_tests.rs`
  - 端到端修复链路
- `apps/cli/tests/cli_smoke.rs`
  - CLI JSON / human-readable / history / backup / restore / prune
- `apps/gui/tests/gui_smoke.rs`
  - GUI Dashboard / Backups / History / export / restore / prune

## Verification evidence

本轮最终核验命令：

```powershell
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

状态：

- `cargo fmt --all --check`：PASS
- `cargo clippy --all-targets --all-features -- -D warnings`：PASS
- `cargo test`：PASS

## Known follow-up items

当前仍明确保留为 follow-up，而不是本版范围内自动修复：

- 不处理认证、账号切换、登录态文件
- 不修复损坏的 SQLite 数据库本体，只处理元数据一致性问题
- 不改写 `history.jsonl` / `logs_2.sqlite` 语义内容，只做存在性/可读性诊断
- release workflow 当前产物命名以 `x86_64` 为主，尚未扩展到更多架构矩阵
- GUI 仍是核心能力的可视化壳层，不提供独立于 `doctor-core` 的额外修复策略

## Handoff summary

可以把当前仓库视为：

- 一版可运行、可测试、可发布的 Codex 本地状态修复基础版本
- CLI / GUI / core 行为已收敛到同一套共享数据契约
- 发布、文档、测试、格式、lint 基本闭环已建立
