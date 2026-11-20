# 0009. Gitflow branching, commit format, and git hooks

- Status: Accepted
- Date: 2026-11-20
- Deciders: Maintainer
- Related: ADR-0001, `AGENTS.md` §6, `.githooks/`

## Context
Several agents from different vendors and humans commit to this repo. Without enforced
conventions, history fills with direct commits to `develop`, vague messages, and tool attribution
noise. The repo already uses `main` (released) and `develop` (integration).

## Decision

### Branches (gitflow)
| Branch | From | Merges into | Purpose |
|---|---|---|---|
| `main` | — | — | Released code only; every merge is tagged `vX.Y.Z` |
| `develop` | `main` | — | Integration branch; always green |
| `feature/<slug>` | `develop` | `develop` | New work; backlog items use `feature/bl-<id>-<slug>` |
| `bugfix/<slug>` | `develop` | `develop` | Non-urgent fixes |
| `release/<x.y.z>` | `develop` | `main` **and** `develop` | Version bump, changelog, release fixes only |
| `hotfix/<x.y.z>` | `main` | `main` **and** `develop` | Urgent production fix |
| `support/<x.y>` | `main` | — | Long-lived maintenance line (only if needed) |

`main` and `develop` change only through reviewed pull requests. Feature PRs target `develop`.
Only humans merge `release/*` and `hotfix/*` into `main` and create tags.

### Commits ([Conventional Commits](https://www.conventionalcommits.org/))
```
<type>(<scope>): <summary>      ← ≤ 72 chars, imperative, no trailing period

<body: why, not what; wrap at 72>
```
- Types: `feat fix perf refactor docs test build ci chore revert`; `!` after type/scope marks a breaking change.
- Scope is the area: `parser`, `runtime`, `http`, `adr`, `harness`, `ci`, …
- One logical change per commit. Formatting-only changes are their own commit.
- **No attribution trailers or footers**: no `Co-authored-by:` lines and no "Generated with …" lines
  for agents or tools, in commits or PR descriptions. The commit author is enough.

### Enforcement
Hooks live in `.githooks/` (plain bash, no dependencies) and are enabled per clone with:
```bash
git config core.hooksPath .githooks
```
| Hook | Enforces |
|---|---|
| `pre-commit` | No direct commits on `main`/`develop` (merges allowed); gitflow branch names; blocks `.env`, `settings.local.json`, keys |
| `commit-msg` | Conventional subject, ≤ 72 chars, blank line 2, no agent attribution |
| `pre-push` | No direct push to `main`/`develop`; `./harness/check.sh --fast` must pass |

`--no-verify` is reserved for humans and must be explained in the PR.

## Consequences
- + Clean, searchable history; changelogs can be generated from commit types.
- + Agents get immediate, local feedback instead of review-time rejections.
- − Hooks are client-side and opt-in per clone; server-side branch protection on GitHub
  (required PR + required CI check on `main`/`develop`) is still needed. Release Engineer owns it.
- − Hooks run from the checked-out tree: on a branch that predates `.githooks/` (or before this
  ADR is merged into `develop`), no hooks run.
- − Pre-push runs the fast gate (~1 min).

## Alternatives considered
- Trunk-based development — simpler, but the repo already has `main`/`develop` and versioned releases.
- `husky`/`pre-commit` frameworks — add a Node/Python dependency for what a few bash lines do.
