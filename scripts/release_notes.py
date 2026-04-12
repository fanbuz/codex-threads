#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Iterable
from urllib import error, parse, request

ISSUE_REF_RE = re.compile(r"#(\d+)")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="为 GitHub Release 生成带 issue 摘要的说明")
    parser.add_argument("--repo", required=True, help="owner/repo")
    parser.add_argument("--tag", required=True, help="当前 release tag，例如 v0.0.2")
    parser.add_argument("--output", required=True, help="输出 markdown 文件路径")
    parser.add_argument("--previous-tag", help="上一个 release tag，用于生成基线说明和提取 commit issue")
    return parser.parse_args()


def github_request(
    repo: str,
    token: str,
    method: str,
    endpoint: str,
    query: dict[str, str] | None = None,
    body: dict | None = None,
) -> dict | list:
    url = f"https://api.github.com/repos/{repo}/{endpoint.lstrip('/')}"
    if query:
        url = f"{url}?{parse.urlencode(query)}"

    data = None
    headers = {
        "Accept": "application/vnd.github+json",
        "Authorization": f"Bearer {token}",
        "X-GitHub-Api-Version": "2022-11-28",
        "User-Agent": "codex-threads-release-notes",
    }
    if body is not None:
        data = json.dumps(body).encode("utf-8")
        headers["Content-Type"] = "application/json"

    req = request.Request(url, data=data, method=method, headers=headers)
    try:
        with request.urlopen(req) as response:
            return json.loads(response.read().decode("utf-8"))
    except error.HTTPError as exc:
        detail = exc.read().decode("utf-8", errors="replace")
        raise RuntimeError(f"GitHub API 请求失败: {exc.code} {detail}") from exc


def normalize_version(tag: str) -> str:
    return tag[1:] if tag.startswith("v") else tag


def find_matching_milestone(repo: str, token: str, version: str) -> dict | None:
    milestones = github_request(
        repo,
        token,
        "GET",
        "/milestones",
        query={"state": "all", "per_page": "100"},
    )
    for milestone in milestones:
        if milestone.get("title") == version:
            return milestone
    return None


def list_closed_issues_for_milestone(repo: str, token: str, milestone_number: int) -> list[dict]:
    issues = github_request(
        repo,
        token,
        "GET",
        "/issues",
        query={
            "state": "closed",
            "milestone": str(milestone_number),
            "sort": "created",
            "direction": "asc",
            "per_page": "100",
        },
    )
    return [issue for issue in issues if "pull_request" not in issue]


def collect_commit_messages(previous_tag: str | None, tag: str) -> str:
    revision_range = f"{previous_tag}..{tag}" if previous_tag else tag
    result = subprocess.run(
        ["git", "log", "--format=%B", revision_range],
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout


def extract_issue_numbers(text: str) -> list[int]:
    numbers = {int(match) for match in ISSUE_REF_RE.findall(text)}
    return sorted(numbers)


def load_issue(repo: str, token: str, number: int) -> dict | None:
    issue = github_request(repo, token, "GET", f"/issues/{number}")
    if "pull_request" in issue:
        return None
    return issue


def collect_referenced_closed_issues(repo: str, token: str, previous_tag: str | None, tag: str) -> list[dict]:
    issues = []
    for number in extract_issue_numbers(collect_commit_messages(previous_tag, tag)):
        issue = load_issue(repo, token, number)
        if issue and issue.get("state") == "closed":
            issues.append(issue)
    return issues


def dedupe_issues(items: Iterable[dict]) -> list[dict]:
    by_number: dict[int, dict] = {}
    for issue in items:
        number = issue.get("number")
        if number is None:
            continue
        by_number[int(number)] = issue
    return [by_number[number] for number in sorted(by_number)]


def render_issue_section(issues: list[dict], milestone_title: str | None) -> str:
    if not issues:
        return ""

    lines = ["## 关联 Issue"]
    if milestone_title:
        lines.append(f"里程碑：`{milestone_title}`")
    for issue in issues:
        lines.append(f"- #{issue['number']} {issue['title']} ({issue['html_url']})")
    return "\n".join(lines)


def generate_base_notes(repo: str, token: str, tag: str, previous_tag: str | None) -> str:
    payload = {"tag_name": tag}
    if previous_tag:
        payload["previous_tag_name"] = previous_tag

    generated = github_request(repo, token, "POST", "/releases/generate-notes", body=payload)
    return generated.get("body", "").strip()


def build_release_notes(repo: str, token: str, tag: str, previous_tag: str | None) -> str:
    version = normalize_version(tag)
    milestone = find_matching_milestone(repo, token, version)

    milestone_issues = []
    milestone_title = None
    if milestone is not None:
        milestone_title = milestone.get("title")
        milestone_issues = list_closed_issues_for_milestone(repo, token, milestone["number"])

    referenced_issues = collect_referenced_closed_issues(repo, token, previous_tag, tag)
    issues = dedupe_issues([*milestone_issues, *referenced_issues])

    base_notes = generate_base_notes(repo, token, tag, previous_tag)
    issue_section = render_issue_section(issues, milestone_title)

    if base_notes and issue_section:
        return f"{base_notes}\n\n{issue_section}\n"
    if base_notes:
        return f"{base_notes}\n"
    if issue_section:
        return f"{issue_section}\n"
    return "暂无额外 release 说明。\n"


def main() -> int:
    args = parse_args()
    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    if not token:
        print("缺少 GH_TOKEN 或 GITHUB_TOKEN", file=sys.stderr)
        return 1

    notes = build_release_notes(args.repo, token, args.tag, args.previous_tag)
    output_path = Path(args.output)
    output_path.write_text(notes, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
