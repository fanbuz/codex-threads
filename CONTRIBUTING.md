# Contributing

Thanks for your interest in contributing to `codex-threads`.

## Before you start

- Open an issue for substantial changes before writing code
- Prefer small, focused pull requests
- Keep new command surfaces agent-friendly and read-first unless there is a strong reason otherwise

## Development workflow

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run the test suite
5. Update documentation when behavior changes
6. Open a pull request with a clear summary

## Local checks

```bash
cargo fmt --all
cargo test
```

If you add or change a command, include:

- help output coverage
- JSON output and search/read behavior coverage
- README command surface updates when relevant

## Issue and release conventions

- 默认使用中文编写 issue 标题、issue 正文和 commit message
- issue 标题只描述任务本身，不要把版本号直接写进标题
- 版本归属通过 GitHub milestone 管理，例如 `0.0.3`
- 提交信息请显式关联 issue 编号，便于 release notes 汇总关联 issue
- issue、文档和对外说明尽量使用自然、顺口、便于理解的表达，避免生硬机械的措辞

## Style

- Keep CLI output stable and machine-readable
- Avoid embedding personal paths, private hosts, or secrets in docs and examples
- Prefer streaming parsers, explicit SQL access paths, and small focused functions

## Pull requests

Please include:

- what changed
- why it changed
- how you tested it
- any follow-up work that remains
