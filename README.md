# codex-threads

[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](./LICENSE)
[![Version: 0.0.4](https://img.shields.io/badge/version-0.0.4-blue.svg)](./Cargo.toml)

`codex-threads` 是一个轻量 Rust CLI，用来把 `~/.codex/sessions` 下的历史 Codex 会话整理成可搜索、可读取的本地索引。

当前版本：`0.0.4`

它面向两类使用方式：

- 人类日常检索旧线程
- Agent 用 `--json` 做结构化回溯

## Project Background

- 本项目聚焦一个简单问题：把本地 Codex 历史线程变成真正可复用的知识索引。
- CLI 设计参考 OpenAI Codex 官方用例文档 [Create a CLI Codex can use](https://developers.openai.com/codex/use-cases/agent-friendly-clis)，重点放在稳定命令面、清晰帮助输出和结构化 JSON。
- 也感谢 [Wangnov/cli-design-framework](https://github.com/Wangnov/cli-design-framework) 提供的 skill 指导与命令行设计思路参考。
- 工具默认读取 `~/.codex/sessions`，把线程、消息和事件增量索引到本地 SQLite，适合脚本和 agent 二次消费。

## Features

- 增量扫描 `~/.codex/sessions`
- 索引健康检查与安全修复入口
- 线程、消息、事件三类搜索与读取接口
- 支持跟随正式版本一起发布、但默认关闭的实验能力
- 默认提供便于直接阅读的命令行输出，以及 `--json` 结构化输出
- 除 `status` / `help` / `--version` 外，命令会附带耗时统计
- SQLite 全文索引优先，必要时回退到普通搜索
- 适合被其他 Codex 线程直接调用

## 安装

直接在仓库根目录运行：

```bash
make install-local
```

或手动安装：

```bash
cargo install --path . --force
```

默认索引目录为 `~/.codex/threads-index`，默认会话目录为 `~/.codex/sessions`。

### Homebrew

```bash
brew tap fanbuz/tap
brew install fanbuz/tap/codex-threads
```

升级：

```bash
brew upgrade codex-threads
```

当前支持平台直接安装预编译二进制：

- macOS arm64
- macOS x64
- Linux x64
- Windows x64

支持平台直接安装预编译二进制，否则回退源码构建；如果当前平台暂时没有对应的预编译包，则需要本地可用的 Rust 工具链。

## 各平台使用说明

### macOS

最省事的方式是直接用 Homebrew：

```bash
brew tap fanbuz/tap
brew install fanbuz/tap/codex-threads
```

升级：

```bash
brew upgrade codex-threads
```

如果你不走 Homebrew，也可以从 GitHub Releases 下载对应平台的预编译包：

- Apple Silicon: `codex-threads-macos-arm64.tar.gz`
- Intel: `codex-threads-macos-x64.tar.gz`

默认目录：

- 会话目录：`~/.codex/sessions`
- 索引目录：`~/.codex/threads-index`

常用命令：

```bash
codex-threads --json sync
codex-threads messages search "build a CLI" --limit 20
codex-threads threads read <session-id> --limit 20
```

### Linux

Linux x64 可以直接从 GitHub Releases 下载：

- `codex-threads-linux-x64.tar.gz`

解压后把二进制放到你的 `PATH` 里即可；如果你更习惯本地构建，也可以在仓库根目录运行：

```bash
cargo install --path . --force
```

默认目录：

- 会话目录：`~/.codex/sessions`
- 索引目录：`~/.codex/threads-index`

常用命令：

```bash
codex-threads --json sync
codex-threads --json messages search "build a CLI" --limit 20
codex-threads events read <session-id> --limit 50
```

### Windows

Windows x64 可以直接从 GitHub Releases 下载：

- `codex-threads-windows-x64.zip`

解压后在 PowerShell 里运行：

```powershell
.\codex-threads.exe --json sync
.\codex-threads.exe messages search "build a CLI" --limit 20
.\codex-threads.exe threads read <session-id> --limit 20
```

如果你想自己构建，可以先安装 Rust 的 MSVC toolchain，然后在仓库根目录运行：

```powershell
cargo install --path . --force
```

默认目录：

- 会话目录：`C:\Users\<you>\.codex\sessions`
- 索引目录：`C:\Users\<you>\.codex\threads-index`

如果你的 Codex 会话不在默认位置，也可以继续用 `--sessions-dir` 和 `--index-dir` 显式覆盖。

### Release Automation

推送 `vX.Y.Z` tag 后，GitHub Actions 会自动：

- 构建并发布 GitHub Release 预编译包
- 向 `fanbuz/homebrew-tap` 发送 `repository_dispatch`
- 由 tap 仓库读取 release 元数据并自动同步 `codex-threads` formula

要启用这条链路，需要在主仓库配置 `HOMEBREW_TAP_TOKEN` secret，用它向 `fanbuz/homebrew-tap` 发送 dispatch 事件。

## Quick Start

```bash
codex-threads --json sync
codex-threads --json doctor
codex-threads --json messages search "build a CLI" --limit 10
codex-threads --json events search "agent_reasoning" --limit 10
codex-threads --json threads read <session-id> --limit 20
```

## 命令

```bash
codex-threads sync
codex-threads --json sync
codex-threads sync --since 2026-04-12T10:30:00Z
codex-threads sync --path session-beta
codex-threads --json sync --recent 20
codex-threads --json sync --budget-files 200
codex-threads --json sync --cooldown 45m
codex-threads sync --force
codex-threads doctor
codex-threads doctor --repair
codex-threads messages search "build a CLI" --limit 20
codex-threads events search "agent_reasoning" --limit 20
codex-threads --json threads search "websocket reconnect"
codex-threads threads read session-alpha --limit 20
codex-threads messages read <session-id> --limit 50
codex-threads events read <session-id> --limit 50
codex-threads status
```

全局参数：

- `--json` 输出纯 JSON
- `--sessions-dir PATH` 覆盖默认会话目录
- `--index-dir PATH` 覆盖默认索引目录

`sync` 范围参数：

- `--since RFC3339` 只同步不早于该时间的会话文件
- `--until RFC3339` 只同步不晚于该时间的会话文件
- `--path PATH` 只同步路径命中该片段的会话文件
- `--recent N` 只同步最近活跃的 N 个会话文件
- `--budget-files N` 单次最多处理 N 个需要刷新的会话文件；超出部分会保存为本地续跑状态，等待下次同参数 `sync` 继续处理
- `--cooldown INTERVAL` 同范围同步的冷却时间，默认 `30m`，支持 `s` / `m` / `h` 单位
- `--force` 忽略冷却时间，立即执行本次同步

`doctor` 参数：

- `--repair` 清理可安全修复的本地状态文件问题，例如过期同步锁、损坏的续跑状态和损坏的冷却状态

搜索过滤参数：

- 三类 `search` 都支持：`--since`、`--until`、`--session`
- `messages search` 额外支持：`--role`
- `threads search` 额外支持：`--cwd`、`--path`
- `events search` 额外支持：`--event-type`

示例：

```bash
codex-threads messages search "CLI" --role user --session session-alpha
codex-threads threads search "search fallback" --cwd alpha-repo --since 2026-04-12T09:00:00Z
codex-threads --json events search "agent" --event-type agent_reasoning --until 2026-04-12T11:00:00Z
```

说明：

- 时间过滤按 RFC3339 时间字符串比较，适合直接复制会话里的时间戳来筛选
- `--cwd` 和 `--path` 是大小写不敏感的模糊匹配
- `--json` 输出会额外回显本次命中的 `filters`，方便脚本和 agent 继续处理
- `--json` 搜索结果还会补充 `search` 元信息，说明这次命中走的是 `fts` 还是 `like`、是否进入 expanded 查询，以及当前排序口径
- 每条搜索结果会带上 `explain`，用来说明命中了哪些字段、覆盖了多少 query term、是否保留了原始字面量命中

## 实验能力

实验能力会跟随正式版本一起发布，但默认关闭，需要在当前命令里显式开启。

开启规则：

- 统一使用 `--enable-experimentals <feature1>,<feature2>`
- 这类开关只对当前命令生效，不会写入长期状态
- 当前只接受白名单 feature 名，不支持未知值、空项或隐式放行

风险分级参考：

- `低`：只读、只解释、或纯 `--dry-run` 的实验能力
- `中`：只修改 `codex-threads` 自己维护的本地状态
- `高`：会改写其他工具或应用的私有本地状态

评估要求：

- 每新增一个实验能力，都需要单独评估风险级别
- 风险提示写在对应能力区块，不使用一段全局结论覆盖所有实验能力

当前实验能力：

### `restore-app-thread`

![跟随版本 0.0.5](https://img.shields.io/badge/%E8%B7%9F%E9%9A%8F%E7%89%88%E6%9C%AC-0.0.5-0A7F5A)

风险级别：`高`

> [!WARNING]
> `restore-app-thread` 会直接修改 Codex App 私有本地状态，例如 `state_5.sqlite`，以及在启用 `--pin` 时修改 `.codex-global-state.json`。
> 这类状态没有稳定公开接口，可能受应用版本、schema 变化和运行中写回影响。建议只在 Codex App 已退出、已完成备份、明确知道恢复目标时使用。

- 读取本地原始 session，并尝试把指定线程恢复到 Codex App 本地线程视图
- 支持 `--dry-run` 先看恢复计划
- 支持 `--pin` 同步把线程加入 `pinned-thread-ids`
- 默认假设 Codex App 已退出，并会在写入前自动创建备份目录

示例：

```bash
codex-threads \
  --enable-experimentals restore-app-thread \
  experimental restore-app-thread <thread-id> \
  --dry-run

codex-threads \
  --enable-experimentals restore-app-thread \
  experimental restore-app-thread <thread-id> \
  --pin
```

输出约定：

- 默认命令行输出会在 `sync`、`search`、`read` 等操作末尾追加 `耗时: ...`
- 耗时会按时长动态显示为 `ms` 或 `s`
- `--json` 模式不输出格式化耗时文本，只提供稳定字段 `duration_ms`
- `sync` 会先输出一段“同步范围”摘要，明确本次时间范围、路径过滤、最近活跃限制和命中的候选文件数
- `sync` 会输出一段“同步冷却”摘要，说明当前冷却间隔、是否命中冷却、最近一次成功刷新时间，以及下次允许刷新时间
- `sync` 在真正执行前会先输出一段同步预检摘要，说明本次检测到的文件规模、变更数量和建议动作
- `--json sync` 会额外返回 `scope`、`cooldown`、`preflight` 和 `resume` 字段，方便 agent 先理解这次同步实际覆盖的范围，以及是应当跳过、继续执行，还是进入续跑
- `sync` 结果会额外回显本次命中的写入策略统计，例如尾部追加、整条重建和回退重建，方便快速判断大 session 是否真的走了增量路径
- 长时间运行的 `sync` 会在 `stderr` 持续输出阶段进度；非交互环境下使用稳定的阶段文本，交互式终端下会退化成单行进度条样式
- `--json sync` 在保留 `stderr` 进度反馈的同时，还会额外返回 `progress` 字段，方便 agent 在结束后读取本次阶段和进度汇总
- 同一个索引目录同一时间只允许一个 `sync` 写任务运行；如果命中活跃锁，新的 `sync` 会直接退出并提示已有同步正在进行
- `sync` 会自动接管超过心跳窗口的过期锁，避免异常退出后的锁文件长期阻塞后续同步
- `status` 会额外展示当前同步锁状态；`--json status` 则会返回 `status.sync_lock` 结构，方便 agent 判断索引目录是否正被同步占用
- `doctor` 会汇总当前索引健康状态，输出问题列表、修复记录和操作建议；`--json doctor` 会返回稳定的 `doctor` 结构，方便 agent 自动消费
- `doctor --repair` 只处理低风险、本地可恢复的问题；像线程计数漂移这类需要重建索引的问题，当前版本会明确提示重新同步而不会自动改写主数据
- 范围化 `sync` 只更新命中范围，不会顺带清理范围外历史；如果要做完整清理，请直接运行不带范围参数的 `sync`
- 当 `--budget-files` 命中预算上限时，本次 `sync` 会返回 `partial=true` 和 `resume.state=saved`，并在索引目录旁写入 `sync.resume.json`
- 后续只要再次用相同参数执行 `sync`，CLI 会自动从 `sync.resume.json` 继续，直到 `resume.state=completed` 后清理这个本地状态文件
- 成功完成一次 `sync` 后，CLI 会在索引目录旁写入 `sync.refresh.json` 记录最近一次刷新时间；同参数的后续 `sync` 默认会在 `30m` 冷却窗口内直接跳过，避免 agent 在短时间内重复刷新

## 设计要点

- 增量同步：仅重建新增或变更过的 `.jsonl` 会话
- 大 session 优化：对持续追加的会话优先走 append-tail 增量刷新，遇到截断或改写时再回退整条重建
- SQLite 索引：线程、消息、事件分表存储
- FTS 优先：若 SQLite 支持 FTS5 则用全文检索，否则自动回退到 `LIKE`
- 线程聚合搜索：按标题、路径、消息内容和事件摘要搜索整条线程

## 代码结构

- `src/commands/`：命令层，负责把 CLI 输入转成具体的搜索、读取和同步调用
- `src/index/types.rs`：索引域的公共数据结构，集中放同步统计、读取结果和搜索结果模型
- `src/index/store/`：索引存储与核心流程
  - `mod.rs`：`Store` 入口、状态汇总与基础统计
  - `resume.rs`：预算化同步的本地续跑状态保存与清理
  - `sync.rs`：同步扫描、会话重建与索引写入
  - `search.rs`：线程、消息、事件三类搜索与过滤逻辑
  - `read.rs`：线程、消息、事件读取逻辑
- `src/parser/`：把 Codex 会话 `jsonl` 解析成线程、消息和事件模型
- `src/output.rs`：文本输出、JSON 响应和耗时展示

这套结构的目标是让同步、搜索、读取、解析和输出各自有稳定落点，避免继续把复杂度堆回单个超大文件。

## 适合的工作流

1. 先运行 `codex-threads --json sync`
2. 用 `messages search`、`threads search` 或 `events search` 找线索
3. 再用 `threads read`、`messages read` 或 `events read` 深入读取

## Contributing

欢迎提交 issue 和 PR。开始之前请先阅读 [CONTRIBUTING.md](./CONTRIBUTING.md)、[CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md) 和 [SECURITY.md](./SECURITY.md)。

建议本地提交前至少运行：

```bash
cargo fmt --all
cargo test
```
