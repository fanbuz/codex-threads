# codex-threads Design

## Goal

构建一个轻量、快速、可脚本调用的 Rust CLI，用来索引、搜索和读取 `~/.codex/sessions` 下的历史 Codex 会话，并为后续线程复用提供稳定 JSON 接口。

## Command Surface

- `codex-threads sync`
- `codex-threads threads search <query> [--limit N]`
- `codex-threads threads read <session-id> [--limit N]`
- `codex-threads messages search <query> [--limit N]`
- `codex-threads messages read <session-id> [--limit N]`
- `codex-threads events read <session-id> [--limit N]`
- `codex-threads status`
- 全局支持 `--json`、`--sessions-dir PATH`、`--index-dir PATH`

## Source Model

`~/.codex/sessions` 中的会话文件为分层目录下的 `.jsonl` 文件。每行包含统一顶层字段：

- `timestamp`
- `type`
- `payload`

当前观察到的核心记录类型：

- `session_meta`
- `response_item`
- `event_msg`
- `turn_context`

线程主标识优先使用 `session_meta.payload.id`，同时保留文件路径和文件名作为辅助检索键。

## Index Strategy

使用 SQLite 作为本地索引存储，索引目录默认放在 `~/.codex/threads-index/`。

数据库包含三类表：

- `files`：记录源文件路径、mtime、size、session_id、同步状态
- `threads`：记录线程级元数据、摘要文本、路径、时间范围
- `messages` / `events`：记录可搜索的消息与事件

搜索优先启用 SQLite FTS5 虚拟表，覆盖线程聚合文本、消息文本和事件文本；若运行环境不支持 FTS5，则自动回退到普通表加 `LIKE` 搜索，保证 CLI 可用性。

## Parsing Rules

- `response_item.payload.type == "message"` 视为消息记录
- 从 `payload.content[]` 中抽取 `text` 或可读文本片段
- `event_msg` 视为事件记录，优先提取 `payload.message`、`payload.text` 或紧凑 JSON 摘要
- `session_meta` 生成线程标题候选：
  - 优先 `payload.cwd` 的末级目录名
  - 再使用首条用户消息摘要
  - 最后回退到 `session_id`

## Output Rules

- 默认输出面向人类阅读，使用稳定列名和短摘要
- `--json` 时输出干净 JSON，不混入说明文本
- 错误统一输出明确原因、建议操作和退出码

## Performance Notes

- `sync` 仅重建新增或变更文件
- 解析按文件流式读取，避免整文件加载
- 索引写入使用事务批量提交

## Skill Packaging

另建 `~/.codex/skills/codex-threads` skill，说明何时触发、如何先执行 `codex-threads --json sync`，以及如何调用 `messages/threads/events` 子命令读取历史线程。
