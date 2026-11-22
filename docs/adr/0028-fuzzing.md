# 0028. Fuzz the parser and evaluator with cargo-fuzz on a schedule

- Status: Accepted
- Date: 2026-11-21
- Deciders: Maintainer
- Related: BL-019, PRD-003 FR-8, BL-017, ADR-0011, threat model 2.1 and 2.2

## Context
Expression text and JSON variables are attacker-controlled (threat model 2.1, 2.2). The rule is
"no panic, no abort, bounded CPU and memory". Hand-written tests (`tests/security_limits.rs`,
`tests/eval_budget.rs`) cover the shapes we thought of. Coverage-guided fuzzing finds the ones we
did not. `cargo-fuzz` needs a nightly toolchain and `libfuzzer-sys`, which must not reach the
published crate, the stable gate or the MSRV build.

## Decision
- **Tool**: `cargo-fuzz` with `libfuzzer-sys` 0.4, dev-only. The crate is `fuzz/`
  (`skillet-fuzz`, `publish = false`) with its own `[workspace]` and its own `fuzz/Cargo.lock`
  (started from the root lock, so the product dependencies have the same versions).
  The root package has no `[workspace]` and its `include` list does not name `fuzz/`, so
  `cargo build --all-targets`, `./harness/check.sh`, the MSRV job and `cargo package` do not see
  it. It depends on `skillet` with `default-features = false`: no QuickJS (it has its own limits,
  ADR-0017, and would slow every run).
- **Targets** (`fuzz/fuzz_targets/`):
  - `parse`: any UTF-8 input into `skillet::parse`.
  - `evaluate`: parse, then `evaluate_parsed_with` over a fixed variable map (every `Value`
    variant) inside `with_limits` with a 1 MiB budget and a 500 ms deadline.
  - `json_vars`: `<expression>\n<json>` into `evaluate_with_json`; without a newline the input
    is the JSON and ten fixed expressions run over it. Same limits.
  An `Err` is always fine. A panic, an abort, a stack overflow, or an evaluation that runs more
  than 2 s past its deadline (the target asserts it) is a bug; libFuzzer's own `-timeout=10`
  catches anything slower.
- **Seeds**: `fuzz/seeds/<target>/` (committed), expressions from `tests/` and the user guide plus
  hand-written JSON cases. The growing corpus `fuzz/corpus/` and `fuzz/artifacts/` are ignored.
- **Builds** use `-a` (debug assertions), so integer overflow panics as it does under
  `cargo test`.
- **Schedule**: `.github/workflows/fuzz.yml`, weekly (Monday 03:17 UTC) plus
  `workflow_dispatch` with a `minutes` input (default 10). One matrix job per target on a pinned
  nightly (`FUZZ_TOOLCHAIN`) and a pinned `cargo-fuzz` version, address sanitizer on (Linux).
  `fuzz/Cargo.lock` is checked with `cargo fetch --locked`. On failure the job uploads
  `fuzz/artifacts/<target>/`. It runs on no push or pull request, so it is not a required check
  and `rust.yml` is unchanged.
- **Alerting** (maintainer, 2026-11-21): when any target fails, a `report` job opens one issue
  labelled `fuzz` and assigned to the repository owner, or comments on the open one, with the run
  link. The issue is closed by hand once the finding is fixed.
- **Findings**: each crash gets a backlog item and, with its fix, a regression test with the
  minimized input in `tests/fuzz_regressions.rs` (stable toolchain, part of the gate).

## Reproducing a crash
```bash
rustup toolchain install nightly-2025-08-04        # the FUZZ_TOOLCHAIN in fuzz.yml
cargo +nightly-2025-08-04 install cargo-fuzz --locked
# download the fuzz-artifacts-<target> artifact from the failed run, then:
cargo +nightly-2025-08-04 fuzz run <target> path/to/crash-<hash>
cargo +nightly-2025-08-04 fuzz tmin <target> path/to/crash-<hash>   # minimize
```
On macOS add `-s none` to every `cargo fuzz` command: the nightly address sanitizer runtime
hangs at start-up on recent macOS. Add `-a` to match CI's debug assertions.

## Consequences
- The parser and evaluator get coverage-guided testing every week without slowing the PR gate.
- The nightly pin and `cargo-fuzz` version need an occasional deliberate bump.
- A red fuzz run does not block merges; someone has to read it. The security reviewer owns that.
- The first local runs found two debug-build panics, both fixed with a one-line bound and pinned
  in `tests/fuzz_regressions.rs`: a lexer subtract overflow on a lone `=` at the start of the
  input (`"=="`), and a negate overflow in `clamp_index` for huge negative indexes
  (`[1][-1e300]`).
