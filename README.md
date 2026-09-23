# codex-threads

[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](./LICENSE)
[![Version: 0.1.1](https://img.shields.io/badge/version-0.1.1-blue.svg)](./Cargo.toml)

在不知道会话 ID 时，从本地 Codex 历史中找到讨论、报错和执行证据。

`codex-threads` 将 `~/.codex/sessions` 整理为本地 SQLite 索引，支持全文检索、离线读取和 JSON 输出。它是 Codex 原生任务能力的历史检索补充层。

## 快速开始

先更新索引。默认有 30 分钟同步冷却；需要立即纳入新内容时使用 `sync --force`。

```bash
codex-threads --json sync
```

检查同步结果的 `partial` 和 `failures`；范围化或部分同步不能当作全部历史已索引。

**找过去的讨论**：记得关键词，但不记得在哪个会话里。

```bash
codex-threads messages search "断线重连" --limit 10
```

**找执行证据**：想知道某个报错当时出现在哪里。

```bash
codex-threads events search "ECONNRESET" --limit 10
```

结果给出候选 `session_id`、命中内容和来源路径。把 ID 交给 Codex 原生能力确认并读取或继续任务；原生访问不可用时，可以离线核对索引里的内容：

```bash
codex-threads messages read <session-id> --limit 20
codex-threads events read <session-id> --limit 20
```

Agent 在命令前加 `--json` 即可获得结构化结果。**未命中只说明当前索引内没有返回结果，不代表全部历史中不存在。** 搜索输出会提示空索引、需重建或指定会话未索引等状态。

## 什么时候使用

- 不知道会话 ID，需要跨历史消息或执行事件搜索：使用本工具。
- 已知 Codex 任务，想读取、续聊、fork 或归档：优先使用原生能力。
- 原生访问不可用，但本地会话或索引还在：使用本地读取兜底。

`threads search` 只查标题、路径和受限的代表性消息，不能代替消息全文检索。`threads context` 保留为确定性历史摘录，不识别最终决策或未完成事项，不保证恢复完整任务状态。

## 安装与升级

```bash
brew tap fanbuz/tap
brew install fanbuz/tap/codex-threads
```

已安装时：

```bash
brew update
brew upgrade codex-threads
codex-threads --version
```

支持平台直接安装预编译二进制，否则回退源码构建；源码构建需要 Rust 工具链。也可运行 `cargo install --path . --force` 或 `make install-local`。

### 从旧版升级

`0.1.1` 沿用 `0.1.0` 的 v2 索引格式，保留已有命令与 JSON 字段，新增检索范围和摘录预算说明字段。无需因本次升级重建索引。

从 `0.0.x` 升级时会清空旧格式的派生索引并要求重建，不修改原始会话。运行一次 `codex-threads --json sync --force`，再用 `codex-threads --json status` 确认 `rebuild_required=false`。旧的实验性私有状态写入功能已移除。

## 各平台使用说明

### macOS

支持 macOS arm64 和 macOS x64，推荐使用上面的 Homebrew 安装方式。也可从 [Releases](https://github.com/fanbuz/codex-threads/releases) 下载 `codex-threads-macos-arm64.tar.gz` 或 `codex-threads-macos-x64.tar.gz`。

### Linux

Linux x64 下载 `codex-threads-linux-x64.tar.gz`，解压后把二进制放入 `PATH`。

macOS / Linux 默认会话目录是 `~/.codex/sessions`，索引目录是 `~/.codex/threads-index`。

### Windows

Windows x64 下载 `codex-threads-windows-x64.zip`，解压后在 PowerShell 运行：

```powershell
.\codex-threads.exe --json sync
.\codex-threads.exe messages search "keyword" --limit 10
```

默认会话目录为 `C:\Users\<you>\.codex\sessions`，索引目录为 `C:\Users\<you>\.codex\threads-index`。各平台均可使用 `--sessions-dir` 与 `--index-dir` 覆盖目录。

## 文档与维护

- [命令参考](docs/commands.md)：过滤条件、同步范围、健康检查、JSON 契约和历史摘录。
- [维护边界与路线图](ROADMAP.md)：本版本范围和后续扩展条件。
- [固定检索验收](docs/retrieval-validation.md)：脱敏样本、预期命中与端到端验证。
- [性能基准](docs/benchmarks/0.1.1.md)：同一快照下的新旧版本对比。
- [本地价值记录模板](docs/value-log.md)：手工记录实际需求，无遥测。

发布流程：推送 `vX.Y.Z` tag 后构建四个平台的二进制并发布 GitHub Release；`fanbuz/homebrew-tap` 的同步工作流更新 formula。版本采用 `0.y.z`，不兼容边界变化增加 `y`，兼容修复增加 `z`。

## Contributing

提交前请阅读 [CONTRIBUTING.md](CONTRIBUTING.md)、[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) 和 [SECURITY.md](SECURITY.md)。本项目优先接受会话格式兼容、检索正确性、索引稳定性和必要的分发修复。

```bash
cargo fmt --all
cargo test --locked
```

CLI 设计参考 [OpenAI 的 Agent-friendly CLI 用例](https://developers.openai.com/codex/use-cases/agent-friendly-clis)，并感谢 [Wangnov/cli-design-framework](https://github.com/Wangnov/cli-design-framework) 的命令行设计思路。
