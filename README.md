# codex-threads

[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](./LICENSE)
[![Version: 0.0.2](https://img.shields.io/badge/version-0.0.2-blue.svg)](./Cargo.toml)

`codex-threads` 是一个轻量 Rust CLI，用来把 `~/.codex/sessions` 下的历史 Codex 会话整理成可搜索、可读取的本地索引。

当前版本：`0.0.2`

它面向两类使用方式：

- 人类日常检索旧线程
- Agent 用 `--json` 做结构化回溯

## Project Background

- 本项目聚焦一个简单问题：把本地 Codex 历史线程变成真正可复用的知识索引。
- CLI 设计参考 OpenAI Codex 官方用例文档 [Create a CLI Codex can use](https://developers.openai.com/codex/use-cases/agent-friendly-clis)，重点放在稳定命令面、清晰帮助输出和结构化 JSON。
- 工具默认读取 `~/.codex/sessions`，把线程、消息和事件增量索引到本地 SQLite，适合脚本和 agent 二次消费。

## Features

- 增量扫描 `~/.codex/sessions`
- 线程、消息、事件三类读取接口
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

支持平台直接安装预编译二进制，否则回退源码构建；如果当前平台暂时没有对应的预编译包，则需要本地可用的 Rust 工具链。

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
codex-threads --json threads read <session-id> --limit 20
```

## 命令

```bash
codex-threads sync
codex-threads --json sync
codex-threads messages search "build a CLI" --limit 20
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

输出约定：

- 默认命令行输出会在 `sync`、`search`、`read` 等操作末尾追加 `耗时: ...`
- 耗时会按时长动态显示为 `ms` 或 `s`
- `--json` 模式不输出格式化耗时文本，只提供稳定字段 `duration_ms`

## 设计要点

- 增量同步：仅重建新增或变更过的 `.jsonl` 会话
- SQLite 索引：线程、消息、事件分表存储
- FTS 优先：若 SQLite 支持 FTS5 则用全文检索，否则自动回退到 `LIKE`
- 线程聚合搜索：按标题、路径、消息内容和事件摘要搜索整条线程

## 适合的工作流

1. 先运行 `codex-threads --json sync`
2. 用 `messages search` 或 `threads search` 找线索
3. 再用 `threads read`、`messages read` 或 `events read` 深入读取

## Contributing

欢迎提交 issue 和 PR。开始之前请先阅读 [CONTRIBUTING.md](./CONTRIBUTING.md)、[CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md) 和 [SECURITY.md](./SECURITY.md)。

建议本地提交前至少运行：

```bash
cargo fmt --all
cargo test
```
