#!/usr/bin/env python3
"""Run a reproducible codex-threads sync/search benchmark without deleting data."""

from __future__ import annotations

import argparse
import json
import subprocess
import time
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--sessions-dir", required=True, type=Path)
    parser.add_argument("--index-dir", required=True, type=Path)
    parser.add_argument("--recent", type=int)
    parser.add_argument("--thread-query", action="append", default=[])
    parser.add_argument("--message-query", action="append", default=[])
    parser.add_argument("--event-query", action="append", default=[])
    parser.add_argument(
        "--reuse",
        action="store_true",
        help="Reuse an existing benchmark index instead of requiring an empty index directory.",
    )
    return parser.parse_args()


def run_json(command: list[str]) -> tuple[dict, float]:
    started = time.perf_counter()
    completed = subprocess.run(command, check=True, capture_output=True, text=True)
    elapsed = time.perf_counter() - started
    return json.loads(completed.stdout), elapsed


def directory_size(path: Path) -> int:
    return sum(item.stat().st_size for item in path.rglob("*") if item.is_file())


def main() -> None:
    args = parse_args()
    binary = args.binary.resolve()
    sessions_dir = args.sessions_dir.resolve()
    index_dir = args.index_dir.resolve()
    index_path = index_dir / "threads.sqlite3"

    if not binary.is_file():
        raise SystemExit(f"binary does not exist: {binary}")
    if not sessions_dir.is_dir():
        raise SystemExit(f"sessions directory does not exist: {sessions_dir}")
    if index_path.exists() and not args.reuse:
        raise SystemExit(
            f"benchmark index already exists: {index_path}; choose an empty directory or pass --reuse"
        )

    index_dir.mkdir(parents=True, exist_ok=True)
    base = [
        str(binary),
        "--json",
        "--sessions-dir",
        str(sessions_dir),
        "--index-dir",
        str(index_dir),
    ]
    sync_command = [*base, "sync", "--force"]
    if args.recent is not None:
        sync_command.extend(["--recent", str(args.recent)])

    sync, sync_wall_seconds = run_json(sync_command)
    queries = []
    for domain, values in (
        ("threads", args.thread_query),
        ("messages", args.message_query),
        ("events", args.event_query),
    ):
        for query in values:
            payload, wall_seconds = run_json(
                [*base, domain, "search", query, "--limit", "20"]
            )
            queries.append(
                {
                    "domain": domain,
                    "query": query,
                    "backend": payload["search"]["backend"],
                    "query_mode": payload["search"]["query_mode"],
                    "count": payload["count"],
                    "duration_ms": payload["duration_ms"],
                    "wall_seconds": round(wall_seconds, 3),
                }
            )

    status, _ = run_json([*base, "status"])
    report = {
        "binary": str(binary),
        "sessions_dir": str(sessions_dir),
        "sessions_bytes": directory_size(sessions_dir),
        "index_dir": str(index_dir),
        "index_bytes": directory_size(index_dir),
        "sync_wall_seconds": round(sync_wall_seconds, 3),
        "sync": sync["stats"],
        "status": status["status"],
        "queries": queries,
    }
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
