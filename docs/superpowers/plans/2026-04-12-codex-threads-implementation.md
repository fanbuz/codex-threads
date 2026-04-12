# codex-threads Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 构建一个 Rust CLI，支持增量索引 `~/.codex/sessions`、搜索线程与消息、读取线程/消息/事件，并封装为可复用的 `codex-threads` skill。

**Architecture:** 使用 `clap` 提供 CLI 入口，`rusqlite` 管理本地索引，解析器按行读取 `.jsonl` 会话文件并将线程、消息、事件拆分入库。输出层统一提供人类可读和 JSON 两种渲染方式，skill 层只封装触发和调用流程。

**Tech Stack:** Rust、clap、serde、serde_json、rusqlite、walkdir、anyhow、tempfile、assert_cmd

---

### Task 1: 初始化工程与测试基线

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/lib.rs`
- Create: `tests/cli_sync.rs`

- [ ] **Step 1: 写失败测试**

```rust
#[test]
fn sync_reports_indexed_files() {
    assert!(false, "replace after scaffolding");
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test sync_reports_indexed_files -- --exact`
Expected: FAIL with assertion failure

- [ ] **Step 3: 初始化最小 CLI 骨架**

创建 `clap` 二进制入口、空的 `sync` 命令和可编译的库结构。

- [ ] **Step 4: 运行测试确认进入绿灯前的真实失败**

Run: `cargo test sync_reports_indexed_files -- --exact`
Expected: FAIL for expected behavior rather than missing project files

### Task 2: 建立会话样本解析器

**Files:**
- Create: `src/parser/mod.rs`
- Create: `src/parser/session.rs`
- Create: `tests/parser_samples.rs`

- [ ] **Step 1: 写失败测试**

验证能从样本 `.jsonl` 中抽取：
- `session_id`
- 线程标题候选
- 消息记录
- 事件记录

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test parser_ -- --nocapture`
Expected: FAIL with missing parser behavior

- [ ] **Step 3: 实现最小解析器**

按行读取 JSONL，识别 `session_meta`、`response_item`、`event_msg`。

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test parser_ -- --nocapture`
Expected: PASS

### Task 3: 建立 SQLite 索引与增量同步

**Files:**
- Create: `src/index/mod.rs`
- Create: `src/index/schema.rs`
- Create: `src/index/store.rs`
- Create: `tests/sync_incremental.rs`

- [ ] **Step 1: 写失败测试**

验证：
- 首次 `sync` 会索引文件
- 第二次 `sync` 不重复写入未变更文件
- 修改源文件后会重新索引

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test sync_ -- --nocapture`
Expected: FAIL with missing store behavior

- [ ] **Step 3: 实现最小索引层**

建立 `files`、`threads`、`messages`、`events` 表，以及必要索引和事务写入。

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test sync_ -- --nocapture`
Expected: PASS

### Task 4: 实现搜索与读取命令

**Files:**
- Create: `src/cli.rs`
- Create: `src/commands/mod.rs`
- Create: `src/commands/sync.rs`
- Create: `src/commands/search.rs`
- Create: `src/commands/read.rs`
- Create: `src/output.rs`
- Create: `tests/search_and_read.rs`

- [ ] **Step 1: 写失败测试**

验证：
- `messages search`
- `threads search`
- `threads read`
- `messages read`
- `events read`
- `--json`

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test search_ read_ -- --nocapture`
Expected: FAIL with unimplemented commands

- [ ] **Step 3: 实现最小命令层**

补齐命令解析、SQLite 查询、JSON 和文本输出。

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test search_ read_ -- --nocapture`
Expected: PASS

### Task 5: 封装 skill 并完成文档

**Files:**
- Create: `README.md`
- Create: `scripts/install.sh`
- Create: `${CODEX_HOME:-~/.codex}/skills/codex-threads/SKILL.md`
- Create: `${CODEX_HOME:-~/.codex}/skills/codex-threads/agents/openai.yaml`

- [ ] **Step 1: 写失败检查**

使用验证脚本检查 skill 结构。

- [ ] **Step 2: 运行检查确认失败或缺失**

Run: `python3 "${CODEX_HOME:-$HOME/.codex}/skills/.system/skill-creator/scripts/quick_validate.py" "${CODEX_HOME:-$HOME/.codex}/skills/codex-threads"`
Expected: FAIL until skill files exist and are valid

- [ ] **Step 3: 实现 skill 与安装脚本**

编写触发说明、调用范式和安装方式。

- [ ] **Step 4: 运行验证确认通过**

Run: `python3 "${CODEX_HOME:-$HOME/.codex}/skills/.system/skill-creator/scripts/quick_validate.py" "${CODEX_HOME:-$HOME/.codex}/skills/codex-threads"`
Expected: PASS

### Task 6: 完整验证

**Files:**
- Modify: `Cargo.toml`
- Modify: `README.md`

- [ ] **Step 1: 运行格式化**

Run: `cargo fmt --all`
Expected: exit 0

- [ ] **Step 2: 运行完整测试**

Run: `cargo test`
Expected: all tests pass

- [ ] **Step 3: 运行手工验证**

Run: `cargo run -- --json sync`
Expected: 返回已索引文件数和会话数

- [ ] **Step 4: 运行手工搜索**

Run: `cargo run -- --json messages search "build a CLI" --limit 5`
Expected: 返回结构化命中结果
