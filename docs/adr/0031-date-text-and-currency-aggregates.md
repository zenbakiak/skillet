# 0031. Dates as ISO 8601 text; Currency in the aggregates

- Status: Accepted
- Date: 2026-11-21
- Deciders: Maintainer
- Related: BL-076, ADR-0027 (amended here), ADR-0003 (`DateTime` is whole Unix seconds,
  `Currency` a tagged `f64`), ADR-0018 D-5 (strict casts), excel-compat B-13 and the
  "string to DateTime cast formats" proposal

> **Currency parts superseded by [ADR-0033](0033-one-numeric-type.md) (2026-11-22).** Skillet has
> one number type, so the aggregates return Numbers. Superseded: decision 3 (B-13), the Currency
> sentence of "Amends ADR-0027", the Currency rows and consequences, and amendment 1 items A3
> (Currency in `.join()`) and A5 (`SUMIF`/`AVERAGEIF`). Decisions 1 and 2 (dates as ISO 8601
> text), A1, A2, A4 and the `NULL` part of A3 stand.

## Context
- A DateTime turned into text as its Unix seconds: `DATE(2026,1,1)::String` was `"1767225600"`,
  and `CONCAT`, `&`, `JOIN` and `.to_s()` wrote the same digits. That text is unreadable, and
  `::DateTime` read it back only because integer text is Unix seconds. `.join()` and `TEXTJOIN`
  rejected a DateTime. excel-compat's cast proposal named ISO 8601 output as a follow-up that
  needs its own ADR.
- `SUM(10::Currency, 5)` was `Number(15)` (excel-compat B-13). ADR-0027 makes
  `10::Currency + 5` `Currency(15)`, so the builtin and the operator disagreed.

## Decision

### 1. A DateTime as text is ISO 8601 UTC
- The text is `YYYY-MM-DDTHH:MM:SSZ` in UTC, whole seconds (a DateTime holds whole seconds):
  `DATE(2026,1,1)` is `"2026-01-01T00:00:00Z"`. A year outside 0000–9999 gets a sign
  (`"+10000-01-01T00:00:00Z"`, `"-0001-…"`). A timestamp outside chrono's range (about ±262,000
  years, reachable only from a host variable) is written as its Unix seconds.
- Every path that turned a DateTime into text uses it (one function, `write_datetime` in
  `src/runtime/datetime.rs`): the `::String` cast, `.to_s()`/`.to_string()` (also inside an
  array), `CONCAT` and `&` (`concat_into`), `JOIN`, and the JSON string `.to_json()` writes.
- `.join()` and `TEXTJOIN` used to reject a DateTime; they now write the same text. `LEN` and
  `FIND` still reject a DateTime (they never turned one into text).
- Not text, so unchanged: the `::Json` cast (a JSON number of seconds), `JQ` and object
  literals (`{"d": DATE(2026,1,1)}` holds a number), the values passed to JS plugins (numbers), and the servers' JSON responses
  (`"type":"DateTime"` with the seconds as a number, or as a string in simple output). None of
  these goes through the text function.

### 2. `::DateTime` reads that text back
- A string cast accepts integer text as Unix seconds (as before) and exactly the form above,
  so `(DATE(2026,1,1)::String)::DateTime == DATE(2026,1,1)`. The year may have a sign and more
  than four digits, so every text the function writes casts back to the same DateTime.
- The cast stays strict (ADR-0018 D-5): no date-only text, no other offsets, no fractions, no
  space separator, no lowercase `t`/`z`, no surrounding spaces, no rollover
  (`"2026-02-30T00:00:00Z"`) and no second 60. Each is `Cannot cast String to DateTime`. The
  other formats in excel-compat's proposal stay a proposal.

### 3. Aggregates keep Currency (B-13)
`SUM`, `PRODUCT` (`MULTIPLY`), `AVERAGE` (`AVG`), `MIN` and `MAX` return Currency when any value
they count (arrays flattened) is Currency; otherwise a Number, as before.
- `SUM` and `PRODUCT` follow ADR-0027: `Currency + Number` and `Currency * Number` are Currency.
- `AVERAGE` is a Currency sum divided by a count, and `Currency / Number` is Currency.
- `MIN` and `MAX` with every value Currency return Currency. With a mix they also return
  Currency, by ADR-0027's rule for a Currency and a Number operand, whichever value is picked:
  `MIN(1, 3::Currency)` is `Currency(1)`. The result type does not depend on which value wins.
- Values the aggregates ignore (text, booleans, `NULL`) do not change the rule. No values gives
  the old result: `SUM()` `0`, `PRODUCT()` `1`, `MIN()`/`MAX()` `0` (Numbers), and `AVERAGE()`
  `#DIV/0!`. Results are not rounded (ADR-0027).
- The lowercase methods (`.sum()`, `.avg()`, `.min()`, `.max()`) are unchanged and return a
  Number. `SUMIF`/`AVERAGEIF` and the statistical builtins are not part of this decision.

## Amends ADR-0027
ADR-0027's Context said "Text output (`::String`, `JOIN`) prints it with 4 decimals"; that is
still true of Currency, and a DateTime's text is now decision 1. ADR-0027 decision 2 (Currency
in the operators) now also sets the result type of the aggregate builtins (decision 3).

## Behaviour changes (ship in 0.7.0)

| Expression | Before | After |
|---|---|---|
| `DATE(2026,1,1)::String`, `.to_s()`, `CONCAT(…)`, `"" & …`, `JOIN([…])` | `"1767225600"` | `"2026-01-01T00:00:00Z"` |
| `DATE(2026,1,1).to_json()` | `Json("\"1767225600\"")` | `Json("\"2026-01-01T00:00:00Z\"")` |
| `[DATE(2026,1,1)].join()`, `TEXTJOIN(",", TRUE, DATE(2026,1,1))` | `Error` | `"2026-01-01T00:00:00Z"` |
| `"2026-01-01T10:30:00Z"::DateTime` | `Error` | `DateTime(1767263400)` |
| `SUM(10::Currency, 5)` | `Number(15)` | `Currency(15)` |
| `PRODUCT(2::Currency, 3)`, `AVERAGE(10::Currency, 20)` | `Number(6)`, `Number(15)` | `Currency(6)`, `Currency(15)` |
| `MIN(1, 3::Currency)`, `MAX(3::Currency, 9::Currency)` | `Number(1)`, `Number(9)` | `Currency(1)`, `Currency(9)` |

## Consequences
- \+ Dates in text are readable and standard, and they round-trip through `::DateTime`.
- \+ Money stays money through `SUM` and friends, as through `+`.
- − Breaking: text built from dates changes, and so does code that compared it to digits or
  parsed it as a number. The old text is `((:d - DATE(1970,1,1)) * 86400)::String`.
- − The servers' JSON still carries Unix seconds, so a DateTime's text inside an expression and
  in a response differ. Changing the wire format would need its own ADR.
- − `MIN`/`MAX` of a mix return Currency even when the picked value was a Number.
