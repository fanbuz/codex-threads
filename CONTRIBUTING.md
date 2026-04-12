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
