# AgentOps Prompt Rules

These rules are the repository source of truth for AgentOps prompts that review or open pull requests. Copy the relevant section into the corresponding agent prompt and keep GitHub issues and PRs as the coordination record.

## Shared GitHub Gate

Before a PASS, ready-for-merge request, or public-surface PR:

1. State the linked issue status.
2. State whether protected public surfaces are touched.
3. State mergeability from GitHub.
4. State CI status.
5. Avoid forbidden public target wording.

Protected public surfaces include `docs/maintainers/launch-kit.md`, release notes, community or social drafts, `SECURITY.md`, `PRIVACY.md`, `LICENSE`, install or release automation, and external announcement copy.

## Quality Gatekeeper

Run protected-surface checks before any PASS verdict.

1. Inspect changed files first.
2. If protected public surfaces are touched, require a linked GitHub issue plus explicit maintainer approval for that exact scope.
3. Do not classify protected-surface PRs as self-contained.
4. If approval or issue linkage is missing, use BLOCK or NEEDS_CHANGES even when CI, build, and tests pass.
5. Every review comment must include linked-issue status, protected-surface status, mergeability, and CI status.

## Growth & Release

Before opening any Growth PR, classify the changed files.

1. For protected public surfaces, open a PR only when there is a linked GitHub issue with explicit maintainer approval or `status/ready-for-agent` for that exact scope.
2. The PR body must include `Closes #<issue>` or `Refs #<issue>`.
3. Explain the user value, developer experience, adoption rationale, maintenance value, or workflow efficiency gained by the change.
4. Keep external posting manual. Do not publish social or community content.
5. If approval is missing, create or update a GitHub issue instead of changing the protected file.

## Parser Builder

Before marking any parser PR ready for review or merge:

1. Fetch latest `master` and update the branch by rebase or merge.
2. Run `gh pr view <PR> --json mergeStateStatus,statusCheckRollup,closingIssuesReferences`.
3. If `mergeStateStatus` is `DIRTY`, `BLOCKED`, or not clearly clean, do not request ready-for-merge. Comment the blocker, update labels to blocked or needs changes, resolve the conflict, and rerun validation.
4. Every parser PR must close or reference exactly one GitHub issue unless a human maintainer explicitly approves a self-contained PR.
5. After any upstream parser PR merges, re-check mergeability before asking Quality to pass the gate.

Parser validation should include:

```bash
cargo test
cargo build --release -p agenttrace
target/release/agenttrace --doctor
```
