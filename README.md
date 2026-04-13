# codex-threads

[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](./LICENSE)
[![Version: 0.0.3](https://img.shields.io/badge/version-0.0.3-blue.svg)](./Cargo.toml)

`codex-threads` 是一个轻量 Rust CLI，用来把 `~/.codex/sessions` 下的历史 Codex 会话整理成可搜索、可读取的本地索引。

当前版本：`0.0.3`

它面向两类使用方式：

- 人类日常检索旧线程
- Agent 用 `--json` 做结构化回溯

## Project Background

- 本项目聚焦一个简单问题：把本地 Codex 历史线程变成真正可复用的知识索引。
- CLI 设计参考 OpenAI Codex 官方用例文档 [Create a CLI Codex can use](https://developers.openai.com/codex/use-cases/agent-friendly-clis)，重点放在稳定命令面、清晰帮助输出和结构化 JSON。
- 工具默认读取 `~/.codex/sessions`，把线程、消息和事件增量索引到本地 SQLite，适合脚本和 agent 二次消费。

## Features

- 增量扫描 `~/.codex/sessions`
- 线程、消息、事件三类搜索与读取接口
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

输出约定：

- 默认命令行输出会在 `sync`、`search`、`read` 等操作末尾追加 `耗时: ...`
- 耗时会按时长动态显示为 `ms` 或 `s`
- `--json` 模式不输出格式化耗时文本，只提供稳定字段 `duration_ms`
- `sync` 会先输出一段“同步范围”摘要，明确本次时间范围、路径过滤、最近活跃限制和命中的候选文件数
- `sync` 在真正执行前会先输出一段同步预检摘要，说明本次检测到的文件规模、变更数量和建议动作
- `--json sync` 会额外返回 `scope` 和 `preflight` 字段，方便 agent 先理解这次同步实际覆盖的范围，以及是应当跳过，还是值得继续执行
- 范围化 `sync` 只更新命中范围，不会顺带清理范围外历史；如果要做完整清理，请直接运行不带范围参数的 `sync`

## 设计要点

- 增量同步：仅重建新增或变更过的 `.jsonl` 会话
- SQLite 索引：线程、消息、事件分表存储
- FTS 优先：若 SQLite 支持 FTS5 则用全文检索，否则自动回退到 `LIKE`
- 线程聚合搜索：按标题、路径、消息内容和事件摘要搜索整条线程

## 代码结构

- `src/commands/`：命令层，负责把 CLI 输入转成具体的搜索、读取和同步调用
- `src/index/types.rs`：索引域的公共数据结构，集中放同步统计、读取结果和搜索结果模型
- `src/index/store/`：索引存储与核心流程
  - `mod.rs`：`Store` 入口、状态汇总与基础统计
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
