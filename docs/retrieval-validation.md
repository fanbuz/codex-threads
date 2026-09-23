# 固定检索验收

样本位于 `tests/fixtures/retrieval`，全部人工构造，不含真实用户会话。预期命中固定如下：

| 搜索域 | 关键词 | 预期会话 | 证据 |
| --- | --- | --- | --- |
| messages | 断线重连 | retrieval-zh | 中文用户问题 |
| messages | websocket reconnect | retrieval-en | 英文用户问题 |
| messages | reconnect_policy | retrieval-zh | assistant 中的代码标识符 |
| events | Error: ECONNRESET | retrieval-zh | function_call_output 报错 |
| events | Error: retry_budget exceeded | retrieval-en | function_call_output 报错 |

每项需唯一命中预期会话，保留字面匹配，并能从 `source.path` 和对应 `read` 输出核对证据。另验证角色、时间范围、会话和事件类型过滤排除非目标记录。

```bash
cargo test --locked --test retrieval_workflow
```

测试会在临时目录创建独立索引，验证以下结果：

- 不知道 ID 时从消息关键词发现讨论，从报错发现工具输出。
- 原始目录不可用时仍能读取已索引的历史证据。
- 三类搜索均能区分空索引、需要重建和指定会话未索引。
- 局部同步与完整同步后均不声称覆盖实时文件的全部变化。
- 中文历史摘录按 UTF-8 字节限制 JSON `text`，结构化消息不受正文预算约束。

## 原生能力兼容快照

2026-09-23 当前 Codex 桌面会话提供原生任务列举、读取、续聊、fork、handoff、归档等接口；当前可调用接口未提供跨全部历史消息/事件的全文搜索入口。这是本次会话接口快照，不代表所有版本或所有界面。

交接时仅将 `handoff.candidate_thread_id` 当作候选，由原生读取确认；不能直接宣称候选是可继续的原生任务。若原生接口不可用，按结果来源与本地 `messages read` / `events read` 核对证据。固定样本只验证候选契约和本地兜底，不依赖远程服务或真实原生任务。
