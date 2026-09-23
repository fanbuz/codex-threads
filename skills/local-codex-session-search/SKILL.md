---
name: local-codex-session-search
description: Search and read local Codex session history under ~/.codex/sessions when the user needs cross-history full-text search, a specific session_id or rollout, offline evidence, or a deterministic history excerpt. Use this only to discover local candidate sessions and hand them to Codex native thread tools. Do not use it to create, fork, continue, message, pin, archive, or otherwise manage Codex threads; those requests belong to native thread tools.
---

# Local Codex Session Search

Use `codex-threads` as a read-only local history index. Treat every result as a candidate until a Codex native thread tool confirms the thread.

## Choose the shortest path

- Known native task / `session_id`: use native thread tools first. Only synchronize and read locally when native access is unavailable or the user explicitly needs local evidence.
- Unknown session: run a bounded sync, then search messages first. Search threads for broader topic matching and events only when execution evidence matters.
- Need a history excerpt: run `codex-threads --json threads context <session-id>` after identifying the candidate. This is deterministic extraction, not task-state recovery; the UTF-8 byte budget only limits JSON `text`, not the whole response.

Inspect `coverage` on search responses. An empty index, an unindexed session, or zero hits is not proof that history does not exist. Search never verifies the whole live corpus. If a bounded search misses, expand synchronization to the relevant range (or the full corpus when requested), preserving directory overrides; inspect sync `partial` and `failures` before interpreting results.

Start with bounded commands unless the user explicitly needs the complete corpus. `--recent` accepts a count of the most recently modified session files, not a duration:

```bash
codex-threads --json sync --recent 30 --budget-files 500
codex-threads --json messages search "exact phrase" --limit 20
codex-threads --json threads search "project topic" --limit 20
codex-threads --json events search "tool or event evidence" --limit 20
```

## Handoff workflow

1. Read the result's `source` and `handoff` fields.
2. Pass `handoff.candidate_thread_id` to Codex native thread tools for confirmation and any continuation or management action.
3. Do not describe a local hit as an active native thread until that confirmation succeeds.
4. If native lookup is unavailable, use `handoff.local_fallback` or `codex-threads --json threads context <session-id>` for deterministic, local-only evidence.

## Boundaries

- Never write Codex App private databases or global state.
- Never substitute this CLI for native create, list, read, continue, fork, handoff, pin, archive, or send-message operations.
- Prefer `--json` for agent workflows; preserve the returned `session_id`, source path, and verification requirement.
- Use event search only for execution evidence. Low-signal lifecycle and duplicated message events are intentionally excluded from the 0.1.0 index.
- A 0.1.0 index-format migration can require one full `codex-threads --json sync --force` before search results are complete.
