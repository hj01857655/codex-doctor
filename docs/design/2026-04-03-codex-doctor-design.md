# codex-doctor 设计文档

## 1. 项目定义
`codex-doctor` 是一个面向 Codex 本地 `.codex` 数据目录的 **跨平台 CLI + GUI 修复工具**。

更准确地说，它是为了解决 **Codex 本地会话状态链失配** 这个问题：历史会话明明仍在磁盘上，但由于 `sessions/`、`archived_sessions/`、`state_5.sqlite`、`config.toml` 以及相关本地状态面不一致，导致会话不可见、索引错乱、元数据冲突，且用户难以安全手工修复。

它的目标不是替代官方 Codex，也不是处理账号认证，而是解决以下本地状态问题：
- provider 切换后历史会话不可见
- `sessions/` 与 `archived_sessions/` 数据错位
- SQLite 状态与 rollout 文件失配
- 配置与索引状态不一致
- 误操作后缺少安全回滚路径

## 2. 设计边界

### 2.1 第一版做什么
- 扫描 `.codex` 关键数据面
- 诊断 rollout / SQLite / config 一致性问题
- 生成修复预览（plan / dry-run）
- 执行修复并产出日志
- 自动备份与恢复
- 提供 CLI 与 GUI 两种入口

### 2.2 第一版不做什么
- 不处理 `auth.json`、登录态、账号切换
- 不代理或修改上游 API 请求
- 不依赖未开源的 Codex IDE 内部实现
- 不改写消息正文、时间戳、标题等语义内容，除非修复动作必须重建元数据

## 3. 已确认的数据面
根据 `openai/codex` 开源源码与文档，第一版重点覆盖：

- `~/.codex/config.toml`
- `~/.codex/sessions/`
- `~/.codex/archived_sessions/`
- `state_5.sqlite`（由 `STATE_DB_VERSION = 5` 推导）
- `history.jsonl`（纳入扫描与备份范围，默认不做语义修复）

另有两个位置需要纳入设计：
- `sqlite_home` / `CODEX_SQLITE_HOME` 的状态库重定向
- `logs_1.sqlite` 的伴随备份策略

## 4. 总体架构

### 4.1 方案
采用 **Rust 核心修复引擎 + CLI + GUI 共用核心能力**。

原因：
- 需要跨平台处理路径、文件锁、SQLite、备份与原子写入
- 需要让 CLI 与 GUI 的诊断/修复结果完全一致
- 未来功能扩展更适合以核心库为中心，而不是复制脚本逻辑

### 4.2 目录规划
```text
E:\VSCodeSpace\codex-doctor
├─ .tmp\codex                    # 仅用于源码分析，不参与发布
├─ apps\cli                      # CLI 入口壳
├─ apps\gui                      # GUI 前端壳
├─ crates\doctor-core            # 扫描、诊断、修复、备份核心
├─ docs\research                 # 外部与源码研究记录
├─ docs\design                   # 总体设计文档
├─ docs\plans                    # 实施计划
├─ docs\compatibility            # 平台与版本兼容结论
└─ tests\fixtures                # 样例 .codex 数据夹具
```

## 5. 核心模块设计

### 5.1 Locator（路径发现）
职责：
- 解析 `CODEX_HOME`
- 解析 `sqlite_home` 与 `CODEX_SQLITE_HOME`
- 发现实际的 sessions、archived、state DB、logs DB、history、config 路径

输出一个统一的 `CodexHomeLayout` 结构，供其他模块复用。

### 5.2 Scanner（扫描器）
职责：
- 枚举 rollout 文件
- 检查 sessions / archived_sessions 目录结构
- 检查 SQLite 文件是否存在、是否可打开
- 汇总 provider 分布、线程数量、归档数量、缺失/损坏文件数量
- 检测 stale path、孤儿 rollout、孤儿 SQLite row、配置缺失等异常

输出 `ScanReport`。

### 5.3 Diagnoser（诊断器）
职责：
- 把原始扫描结果归并为用户可理解的问题列表
- 给出每个问题的证据、风险级别、候选修复动作

首批问题类型：
- `MissingSqliteThreadRow`
- `StaleSqliteRolloutPath`
- `RolloutProviderMismatch`
- `ArchivedStateMismatch`
- `MissingRootModelProvider`
- `MissingSessionsDirectory`
- `UnreadableSqliteDatabase`
- `LockedDatabase`
- `LockedRolloutFile`

输出 `DiagnosisReport`。

### 5.4 Repair Planner（修复计划器）
职责：
- 将诊断结果映射为最小修复动作集合
- 支持 dry-run / plan 输出
- 支持按问题类型启停特定修复器

修复动作抽象：
- `RewriteRolloutSessionMeta`
- `UpsertSqliteThreadMetadata`
- `MoveRolloutToArchive`
- `MoveRolloutToSessions`
- `PatchConfigModelProvider`
- `RebuildMissingIndexFromRollout`

输出 `RepairPlan`。

### 5.5 Executor（执行器）
职责：
- 按顺序执行修复动作
- 在执行前创建备份快照
- 控制写入顺序与回滚点
- 产出结构化执行日志

执行规则：
1. 先备份
2. 再修 config / 文件 / SQLite
3. 每一步记录变更摘要
4. 遇到锁冲突时保留已完成结果并给出可重试项

### 5.6 Backup / Restore（备份与恢复）
职责：
- 创建带时间戳的备份目录
- 记录元信息（来源路径、版本、修复动作）
- 支持列出备份、恢复指定备份、清理旧备份

约束：
- 只管理本工具自己生成的备份
- 默认保留最近 N 份
- 恢复前要求目标路径状态校验

## 6. CLI 设计

### 6.1 命令集（第一版）
- `codex-doctor scan`
- `codex-doctor diagnose`
- `codex-doctor repair`
- `codex-doctor backup list`
- `codex-doctor backup restore <id>`
- `codex-doctor backup prune`

### 6.2 关键参数
- `--codex-home`
- `--sqlite-home`
- `--provider <name>`
- `--dry-run`
- `--json`
- `--include-archived`
- `--keep <n>`
- `--fix <problem-type>`

### 6.3 输出原则
- 默认输出给人看得懂的摘要
- `--json` 输出结构化报告，便于 GUI / 自动化复用
- 修复后明确列出：已修复 / 跳过 / 失败 / 需要重试

## 7. GUI 设计

### 7.1 第一版界面结构
- 顶部：Codex Home / SQLite Home 路径
- 中部左侧：扫描摘要（sessions、archived、SQLite、provider 分布）
- 中部右侧：问题列表（按严重级别分组）
- 底部：修复计划预览、执行日志、备份列表

### 7.2 关键交互
- `Refresh`：重新扫描
- `Preview Repair`：生成修复计划，不落盘
- `Execute Repair`：执行修复
- `Restore Backup`：从历史备份恢复
- `Prune Backups`：清理旧备份

### 7.3 GUI 原则
- GUI 只是核心库的可视化封装，不拥有独立修复逻辑
- 所有危险动作都要二次确认
- 默认先预览，再允许执行

## 8. 数据一致性原则
- **配置、rollout、SQLite 必须一起看**，不能只修单点
- 任何“猜测性修复”都必须在报告中注明依据
- 如果证据不足，默认只报告，不自动改写
- 若目标文件/数据库被占用，优先安全退出或跳过，并在结果中明确指出

## 9. 错误处理策略
- 锁冲突：标记为 `retryable`
- 文件损坏：保留原件并输出隔离副本路径
- SQLite 无法打开：不做写入，只给诊断与建议
- 局部修复失败：不影响已成功的其他修复项；但要输出完整结果

## 10. 测试策略

### 10.1 单元测试
覆盖：
- 路径发现
- rollout 元数据提取
- provider / archived / stale path 诊断逻辑
- 备份命名与保留策略

### 10.2 集成测试
基于 `tests/fixtures` 构造多类 `.codex` 样例：
- 正常数据
- rollout 存在但 SQLite 缺行
- SQLite 行存在但 rollout_path 失效
- archived 状态不一致
- config 缺失 provider
- 锁文件 / 占用模拟

### 10.3 GUI 验证
- 至少验证扫描、预览、执行、恢复四条主路径
- GUI 的结果必须与 CLI `--json` 输出一致

## 11. 第一版里程碑

### M1：研究与数据模型固化
- 形成 `.codex` 路径与数据结构结论
- 明确 state / rollout / config 的修复边界

### M2：核心扫描与诊断
- 完成 `doctor-core` 的 layout、scan、diagnose
- 先只输出报告，不做写入

### M3：备份与修复引擎
- 支持 dry-run、执行、恢复、清理
- 覆盖 provider / stale path / missing row / archived mismatch / config mismatch

### M4：CLI 可用
- 命令集稳定
- 产出机器可读 JSON

### M5：GUI 可用
- 支持 scan / preview / execute / restore
- 封装核心能力，不分叉逻辑

## 12. 当前结论
`codex-doctor` 第一版应当被设计成一个**本地状态修复系统**，而不是“某个脚本工具的升级版”。

它的成功标准不是只修好 provider，而是能稳定覆盖：
- provider 问题
- 索引 / 路径问题
- archived / active 状态错位
- SQLite 元数据缺失或陈旧
- config 与本地状态不一致
- 整体备份与回滚链路
