# 0016. Custom functions are visible in every sub-expression

- Status: Accepted (2026-11-21, option A)
- Date: 2026-11-20
- Deciders: Language Designer (proposal), Maintainer
- Related: BL-031, BL-030, ADR-0007 (`CustomFunction` + JS plugins), `src/runtime/evaluator.rs`

## Context
Custom functions are resolved through `EvaluationContext::get_custom_registry()`. BL-030 made the
existing visibility rules explicit: `Scope::new(ctx, registry)`. Only `.map/.filter/.reduce/...`
lambdas (`method_calls/lambda_methods.rs:28`) pass the registry through. Plain method arguments
(`eval_arg`, evaluator.rs:150) and the builtin lambdas `FILTER`, `FIND`, `MAP`, `REDUCE`, `SUMIF`,
`AVGIF` and `COUNTIF` (seven `Scope::new(context, None)` sites) pass `None`. A further gap:
`evaluate_with_assignments*` in `src/lib.rs` never attach the registry. So any formula containing
`:=` or `;` loses custom functions in `sk`, `sk_server` and `sk_http_server`, all of which route
those formulas there.

Measured on the release build. `sk` auto-loads the JS plugin `hooks/mx_isr.js`, and the shorthand
`F` stands for `MX_ISR(21000, "semanal").isr`:

```
=F                                         -> Number(5025.976)
=SUM(F, 1) / =IF(TRUE, F, 0) / =[F]        -> Number(5026.976) / Number(5025.976) / Array([...])
=[21000].map(MX_ISR(:x, "semanal").isr)    -> Array([Number(5025.976)])
=[21000].filter(MX_ISR(:x, "semanal").isr > 0) -> Array([Number(21000.0)])
=[[21000]].map(:x.map(MX_ISR(:x, "semanal").isr)) -> Array([Array([Number(5025.976)])])
=MAP([21000], MX_ISR(:x, "semanal").isr)   -> Error: Unknown function: MX_ISR
=FILTER([21000], MX_ISR(:x, "semanal").isr > 0) -> Error: Unknown function: MX_ISR
=REDUCE / SUMIF / FIND (same lambda)       -> Error: Unknown function: MX_ISR
=(2.5).round(F)                            -> Error: Unknown function: MX_ISR
=[21000].map(:x.round(F))                  -> Error: Unknown function: MX_ISR
=[21000].map(FILTER([1], F > 0))           -> Error: Unknown function: MX_ISR
=:v := F; :v      /  =:v := 1; F           -> Error: Unknown function: MX_ISR
```

The comment on `Scope` says these sites "deliberately" hide the registry, but no ADR, PRD or doc
gives a reason. It is an accident of the old duplicate evaluator (BL-001). No other language with
user-defined functions makes their visibility depend on syntactic position (Excel LAMBDA/named
functions, Ruby methods, SQL UDFs).

## Options

| Expression | Current | A: visible everywhere | B: keep, document |
|---|---|---|---|
| `[21000].map(MX_ISR(:x,"semanal").isr)` | works | works | works |
| `MAP([21000], MX_ISR(:x,"semanal").isr)` | Unknown function | works | Unknown function |
| `FILTER([21000], MX_ISR(:x,"semanal").isr > 0)` | Unknown function | `[21000]` | Unknown function |
| `(2.5).round(F)` | Unknown function | evaluates (`F` as precision) | Unknown function |
| `[21000].map(:x.round(F))` | Unknown function | works | Unknown function |
| `:v := F; :v` (custom-enabled entry) | Unknown function | `5025.976` | Unknown function |
| `FILTER([1,2], SUM(:x) > 1)` with `SUM` overridden | builtin `SUM` | **custom `SUM`** | builtin `SUM` |

- **A. One namespace.** Every context that has a registry passes it into every child scope. In
  practice: `Scope::new(ctx, ctx.get_custom_registry())` at all eight sites, which lets the
  `registry` field and parameter disappear. `evaluate_with_assignments*` use `GLOBAL_REGISTRY`, as
  `evaluate_with_custom` already does. The non-custom entry points (`evaluate`, `evaluate_with`)
  stay registry-free. Signatures are unchanged.
- **B. Keep the current rules and document them.** Nothing changes. Users must learn which
  positions see custom functions, and there is no principle that explains the positions.
- **C. Partial: fix the builtin lambdas only.** Removes the `.map` vs `MAP` asymmetry but leaves
  method arguments and `:=`. Half a rule is still a rule to document.

## Decision
Adopt **A**. The visibility rule becomes: "the entry point decides. `evaluate_with_custom`,
`evaluate_with_json_custom` and the assignment entry points see registered functions in every
sub-expression. `evaluate` and `evaluate_with` see none." PRD-001 gains one FR row with that sentence.
Non-Turing-completeness (AGENTS §7) is unaffected. A custom function cannot call an expression back,
so wider visibility adds no recursion.

## Compatibility
- Changed: formulas that errored with `Unknown function` now evaluate. The single silent change is a
  custom function that **shadows a builtin** (`tests/custom_functions.rs::test_custom_function_priority_over_builtin`)
  and is used inside `FILTER/MAP/REDUCE/SUMIF/AVGIF/COUNTIF/FIND` lambdas, method arguments or `:=`
  formulas. There it now resolves to the custom function, which is consistent with top level.
  Changelog note.
- `evaluate_with_assignments*` gain custom functions through `GLOBAL_REGISTRY`. Embedders that
  relied on assignments being registry-free must call a registry-free variant. None exists today,
  so the Rust Engineer decides whether one is needed. If one is added, it needs a `#[deprecated]` path,
  not removal.
- Tests: nothing in `tests/` asserts `Unknown function` in these positions (`custom_functions.rs`,
  `evaluator_parity.rs` Mode::Custom cases all use top-level or `.map` calls). Docs: none describe the
  restriction.
- QA: column A of the table becomes `tests/custom_functions.rs` cases with a Rust `CustomFunction`
  (not the JS plugin), so they run with `--no-default-features`.

## Consequences
- + One rule, explained in one sentence. Removes the `registry` parameter from `Scope`.
- − A JS plugin is now reachable from more positions in a formula, so ADR-0011 budget and timeout
  enforcement must cover those call paths. They already share the single function-call branch in
  `Evaluator::eval` (`eval_function_call`, evaluator.rs ~L500), so no new path is added.
