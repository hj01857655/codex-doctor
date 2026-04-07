# codex-doctor 实现方案说明

## 1. 问题定义

`codex-doctor` 要解决的问题不是单个 session 文件损坏，而是 **Codex 本地多源状态失配**：

- `sessions/` 与 `archived_sessions/` 目录状态错位
- rollout 文件与 `state_5.sqlite` 元数据不一致
- `config.toml` 的 root provider 与本地状态不一致
- `logs_2.sqlite` / `history.jsonl` 缺失或不可读
- SQLite / rollout 文件被占用，导致修复无法安全执行

这些问题会表现为：

- session 不可见
- 归档状态异常
- provider 显示错误
- repair 失败且用户不知道下一步该做什么

## 2. 根因分析

通过对上游源码和本地数据面的分析，确认 Codex 会话状态由多份本地数据共同维持：

- `config.toml`
- `sessions/`
- `archived_sessions/`
- `state_5.sqlite`
- `logs_2.sqlite`
- `history.jsonl`
- `sqlite_home / CODEX_SQLITE_HOME` override

上游正常路径默认这些数据源保持一致；一旦发生漂移，上游并没有专门的 doctor / recovery 层来：

- 解释根因
- 生成最小修复动作
- 先备份再修复
- 标记锁冲突为可重试

## 3. 产品目标

`codex-doctor` 被实现为一个 **Codex 本地状态诊断与修复工具**，而不是普通脚本集合。

产品目标：

1. 发现本地状态布局
2. 扫描关键数据面
3. 诊断一致性问题
4. 生成可解释的修复计划
5. 在安全备份前提下执行修复
6. 支持恢复与历史追踪
7. 同时提供 CLI 和 GUI 两个入口

## 4. 非目标

当前明确不做：

- 认证 / 账号切换
- 上游 API 请求代理
- 消息正文与语义内容改写
- 已损坏 SQLite 内容级恢复
- 独立于 `doctor-core` 的第二套 GUI 修复逻辑

## 5. 解决方案架构

### 5.1 共享核心

`crates/doctor-core` 负责所有核心行为：

- layout discovery
- scan
- diagnose
- repair planning
- repair execution
- backup / restore
- repair history

CLI 和 GUI 都只消费这层能力。

### 5.2 CLI

`apps/cli` 提供：

- `scan`
- `diagnose`
- `repair`
- `backup list / restore / prune`
- `history`

支持：

- human-readable 输出
- JSON 输出
- retryable next-step hint

### 5.3 GUI

`apps/gui` 提供：

- Dashboard
- Backups
- History

并复用同一套 core API，而不是通过 subprocess 调 CLI。

## 6. 已实现能力

### 6.1 布局发现

已实现：

- `config.toml`
- `sessions/`
- `archived_sessions/`
- `state_5.sqlite`
- `logs_2.sqlite`
- `history.jsonl`
- `sqlite_home` override

### 6.2 诊断能力

已实现问题码：

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

### 6.3 修复动作

已实现修复动作：

- `RebuildMissingIndexFromRollout`
- `UpsertSqliteThreadMetadata`
- `MoveRolloutToArchive`
- `MoveRolloutToSessions`
- `RewriteRolloutSessionMeta`
- `PatchConfigModelProvider`

### 6.4 锁冲突处理

已实现：

- scan 阶段检测 locked database / locked rollout
- repair 阶段将锁冲突转换为 `skipped + retryable`
- backup 阶段若被锁资源阻塞，会整批动作转为 `retryable`
- CLI / GUI 输出重试提示
- repair history 持久化 `retryable`

### 6.5 安全能力

已实现：

- backup snapshot
- backup list
- backup restore
- backup prune
- repair history save/list

## 7. 模块映射

- `crates/doctor-core/src/layout.rs`
  - 本地状态路径发现
- `crates/doctor-core/src/scan.rs`
  - 扫描与锁探测
- `crates/doctor-core/src/diagnose.rs`
  - 问题归类
- `crates/doctor-core/src/plan.rs`
  - 诊断结果到修复动作映射
- `crates/doctor-core/src/repair.rs`
  - 备份前修复执行、retryable 处理
- `crates/doctor-core/src/backup.rs`
  - 备份/恢复/清理
- `crates/doctor-core/src/history.rs`
  - 修复历史记录
- `apps/cli/src/main.rs`
  - CLI 命令绑定与 JSON 输出
- `apps/cli/src/output.rs`
  - CLI 人类可读输出
- `apps/gui/src/lib.rs`
  - GUI ViewModel 与交互逻辑

## 8. 验证方式

当前以这些命令作为主验收：

```powershell
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

同时覆盖：

- core unit/integration tests
- CLI smoke tests
- GUI smoke tests

## 9. 已知边界

当前仍然保留这些边界：

- 不修复内容级数据库损坏
- 不接管认证与账号问题
- 不改写 `history.jsonl` / `logs_2.sqlite` 的业务语义
- 更偏“本地状态 doctor”，不是完整 session 管理器

## 10. 当前产品定位

一句话定义：

> `codex-doctor` 是一个面向 Codex 本地多源状态失配问题的诊断、修复、回滚与追踪工具。

它解决的是“源码正常路径之外，本地状态坏了以后怎么发现、怎么解释、怎么安全修回去”的问题。
