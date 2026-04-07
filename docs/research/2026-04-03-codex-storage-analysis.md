# Codex 本地存储与修复面研究（2026-04-03）

## 研究目标
为 `codex-doctor` 确认可安全修复的本地数据面，避免依赖未开源的 IDE 包装层。

## 已核对的外部前提
- `openai/codex` 是开源仓库，适合直接分析本地数据结构与状态流。
- Codex IDE / VS Code 扩展当前**不是**开源；`openai/codex` issue `#4352` 关闭为 `not planned`。
- 因此第一版应围绕 `.codex` 目录与本地 SQLite 状态库实现，而不是反向依赖 IDE 内部逻辑。

## 从源码确认的关键事实

### 1. 配置入口
- `docs/config.md` 明确说明配置文件位于 `~/.codex/config.toml`。
- MCP、日志、SQLite 状态等行为都与 `config.toml` / `CODEX_HOME` 相关。

### 2. SQLite 状态库位置与版本
- `codex-rs/state/src/lib.rs` 定义：
  - `SQLITE_HOME_ENV = "CODEX_SQLITE_HOME"`
  - `STATE_DB_FILENAME = "state"`
  - `STATE_DB_VERSION = 5`
- `codex-rs/state/src/runtime.rs` 中的 `state_db_path()` 会组合出：
  - `state_5.sqlite`
- `docs/config.md` 说明 SQLite 状态库位于 `sqlite_home` 配置项或 `CODEX_SQLITE_HOME` 环境变量下；未显式配置时通常回落到 `CODEX_HOME`。

### 3. 会话文件目录
- 活跃 rollout 位于 `sessions/`
- 归档 rollout 位于 `archived_sessions/`
- `codex-rs/rollout/src/list.rs` 与 `codex-rs/app-server/README.md` 都验证了这两个目录是正式的数据面。

### 4. rollout 文件里的 provider 元数据
- `codex-rs/rollout/src/metadata.rs` 中 `builder_from_session_meta()` 会把
  `session_meta.meta.model_provider` 写入线程元数据构建器。
- 这说明 rollout JSONL 本身持有 provider 相关信息，不能只看 SQLite。

### 5. SQLite 线程元数据里的 provider 字段
- `codex-rs/state/src/model/thread_metadata.rs` 的 `ThreadRow` 包含：
  - `rollout_path`
  - `model_provider`
  - `archived_at`
  - `cwd`
  - `git_*`
- 这意味着“历史会话不可见”或“索引异常”很可能是 rollout 与 SQLite 元数据失配，而不是单一文件问题。

### 6. 线程路径查找优先使用 SQLite
- `codex-rs/rollout/src/list.rs` 的 `find_thread_path_by_id_str_in_subdir()`：
  - 先查 SQLite 中记录的 rollout 路径
  - 再回退到文件系统扫描
  - 若 SQLite 返回的路径不存在，会记录 `stale_db_path` 异常
- 这正是 `codex-doctor` 需要覆盖的诊断/修复场景之一。

### 7. 归档恢复是正式能力
- `codex-rs/app-server/README.md` 公开了 `thread/unarchive`
- `codex-rs/app-server/tests/suite/v2/thread_unarchive.rs` 验证：归档 rollout 可以被移回 `sessions/`
- 因而“归档恢复”可作为 `codex-doctor` 的一等功能，而不是实验性 hack。

### 8. 缺失 SQLite 行可被修复
- `codex-rs/app-server/tests/suite/v2/thread_metadata_update.rs` 包含：
  - `thread_metadata_update_repairs_missing_sqlite_row_for_stored_thread`
  - `thread_metadata_update_repairs_missing_sqlite_row_for_archived_thread`
- 这说明 Codex 自身已经承认“rollout 存在但 SQLite 行缺失”是可修复状态。

### 9. 额外相关数据面
- `docs/tui-chat-composer.md` 说明持久化输入历史位于 `~/.codex/history.jsonl`
- 第一版不应修改该文件的语义内容，但扫描时可以将其纳入存在性与备份范围。

## 对 codex-doctor 的直接影响
1. 修复核心必须同时覆盖 **rollout + SQLite + config**
2. 必须支持 **active / archived** 双目录
3. 必须具备 **stale rollout_path** 与 **missing sqlite row** 诊断能力
4. 任何写操作前都要先做 **可恢复备份**
5. 第一版不碰认证、账号切换、远程服务协议，只处理本地数据一致性

## 仍待后续验证的点
- SQLite 里 `threads` 之外是否还有必须同步的派生表
- `logs_1.sqlite` 是否需要纳入备份但排除在修复之外
- 被活跃进程占用时，Windows/macOS/Linux 上的锁冲突表现是否一致
- GUI 首版是否需要暴露高级“逐项修复开关”，还是仅支持预览 + 全量应用

## 结论
`codex-doctor` 的正确切入点不是复刻某个 UI，而是把 `.codex` 当作一个
“可扫描、可诊断、可备份、可修复、可回滚”的本地状态系统来处理。
参考 `openai/codex` 开源 CLI 源码即可覆盖核心数据面，IDE 私有壳层不应成为首版依赖。
