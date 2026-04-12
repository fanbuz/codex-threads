# codex-threads Homebrew Dispatch Migration Design

## Goal

把 `codex-threads` 的 Homebrew 自动发布链路迁移为“主项目仓库只发 release 并通知 tap，tap 仓库自己完成 formula 同步”的结构，减少主项目仓库里的 Homebrew 专用实现。

## Current State

- `codex-threads` 主仓库当前在 release workflow 中直接 checkout `fanbuz/homebrew-tap`、渲染 formula、commit 并 push tap。
- 主仓库还包含项目专用的 formula 渲染脚本与测试。
- `homebrew-tap` 已有通用 `sync-formula.yml`、`scripts/render_formula.py` 和 `FormulaSpec/gitea-cli.json`，可以作为发布中枢复用。

## Target Architecture

- `codex-threads` 主仓库：
  - 负责构建并发布 GitHub Release 资产
  - release 成功后用 `repository_dispatch` 通知 `fanbuz/homebrew-tap`
  - payload 至少包含 `source_repository`、`tag`、`formula_name`
- `homebrew-tap` 仓库：
  - 接收 `repository_dispatch` 或手动 `workflow_dispatch`
  - 读取 source repo release 元数据
  - 使用通用渲染脚本和 `FormulaSpec/codex-threads.json` 生成 formula
  - 提交并 push tap 变更

## Scope

- 保持 `codex-threads` 现有 release 资产命名与 Homebrew 安装路径不变
- 迁移后继续支持：
  - macOS arm64
  - macOS x64
  - Linux x64
- 不在主项目仓库继续保留 formula 渲染脚本和 tap push 逻辑

## Key Changes

### codex-threads

- release workflow 改为：
  - `build`
  - `publish`
  - `notify-homebrew-tap`
- 删除项目内专用 `render_homebrew_formula.py`
- 删除或替换与项目内渲染脚本绑定的测试
- README 改为说明“release 后通知 tap 自动同步”

### homebrew-tap

- 复用现有 `sync-formula.yml`
- 复用现有 `scripts/render_formula.py`
- 新增 `FormulaSpec/codex-threads.json`
- 如有必要，在通用渲染脚本中兼容 `macos_x64` / `macos_amd64` 这类配置键差异

## Testing Strategy

- `codex-threads`
  - 先补失败测试，断言 workflow 使用 `repository_dispatch`
  - 断言使用 `HOMEBREW_TAP_TOKEN`
  - 断言 payload 包含 `source_repository`、`tag`、`formula_name`
- `homebrew-tap`
  - 先补失败测试，断言 `sync-formula.yml` 同时支持 `repository_dispatch` 和 `workflow_dispatch`
  - 断言通用渲染脚本可以根据 `FormulaSpec/codex-threads.json` 正确渲染 formula

## Verification

- `codex-threads`: `cargo test --locked`，必要时 `cargo fmt --all --check`
- `homebrew-tap`: `python3 -m unittest discover -s tests`
- 两边 workflow 做 YAML 语法检查
- 本轮只做本地改造与验证，不推远端
