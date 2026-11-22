# 0031. Dates as ISO 8601 text; Currency in the aggregates

> **Amendment 1 (maintainer, 2026-11-21, BL-078):** five follow-ups, all accepted as recommended.
> Status stays Accepted. Results below are what the BL-078 implementation must give; error
> positions count bytes from the start of the caller's input, including a leading `=` and any
> spaces before it (BL-069).
>
> **A1. Casts chain, left to right.** A cast may follow a cast: `x::A::B` is `(x::A)::B`. The
> postfix rule becomes `atom link* cast*`, where a link is `.name`, `.name(…)`, `&.name`, `[i]` or
> `[a:b]` and a cast is `:: TypeName`. Everything else about precedence stays as it is today:
> - the casts apply to the whole link chain before them: `:u&.name::String::Integer` is
>   `((:u&.name)::String)::Integer`;
> - a link after a cast is still an error; use parentheses. `1::String.length()` and
>   `1::String::Integer[0]` stay `Unexpected token after end of expression` (position of the
>   `.` or `[`); `(1::String).length()` is `1`;
> - prefix `-`, `+` and `!` apply to the whole cast chain: `-"2"::String::Integer` is
>   `-(("2"::String)::Integer)`; so does `NOT`, and a `^` operand is one such chain;
> - the percent suffix still cannot follow a cast (`10%::String` stays an error);
> - every cast counts one level against `MAX_NESTING_DEPTH` (128), like a link.
>
> | Expression | Result |
> |---|---|
> | `DATE(2026,1,1)::String::DateTime` | `DateTime(1767225600)` |
> | `DATE(2026,1,1)::String::DateTime == DATE(2026,1,1)` | `true` |
> | `1::String::Integer + 1` | `2` |
> | `"2"::Integer::String` | `"2"` |
> | `1.5::Currency::String` | `"1.5000"` (as `(1.5::Currency)::String` today) |
> | `NULL::String::Integer` | `0` (`""::Integer` is `0` today) |
> | `1::String::Integer::Float` | `1` |
> | `-"2"::String::Integer` | `-2` |
> | `2^"3"::String::Integer` | `8` |
> | `NOT 1::String::Boolean` | `false` (as `NOT ("1"::Boolean)`) |
> | `:u&.name::String::Integer` with `{"u": null}` | `0` |
> | `=1::String::Foo` | `Error: Unknown cast type at position 12` |
> | `  =1::String::Foo` | `Error: Unknown cast type at position 14` |
> | `=1::Foo` | `Error: Unknown cast type at position 4` (today: no position) |
> | `=1::String::5` | `Error: Expected type name after '::' at position 12` |
> | `=1::String::` | `Error: Expected type name after '::' at position 10` (end of input: the parser's usual end-of-input position, the start of the last token, as `=1 +` gives 3) |
> | `=1::String.length()` | `Error: Unexpected token after end of expression at position 10` (unchanged) |
> | `1` followed by 128 `::String` / by 129 | `"1"` / `Error: Expression nested too deeply (max 128) at position …` (as 128 / 129 `.upper()` links on `"a"` today) |
>
> Both cast errors get the position of the token after `::` (the type name, or what stands where
> it should be), for the first cast and every later one. The messages do not change.
>
> **A2. `.to_json()` on a date stays the ISO text.** No change: `DATE(2026,1,1).to_json()` is
> `Json("\"2026-01-01T00:00:00Z\"")`, and the `::Json` cast stays a number,
> `DATE(2026,1,1)::Json` is `Json("1767225600")`. The two differ on purpose: `.to_json()` writes
> the value's text, `::Json` its data (decision 1).
>
> **A3. `.join()` accepts Currency and `NULL`, as `JOIN` does.** A Currency item is written with
> 4 decimals and `NULL` as nothing, exactly as `JOIN` writes them today. Only those two change.
> The other items keep the method's Ruby text (ADR-0027: methods keep Ruby semantics): booleans
> stay `true`/`false` (`JOIN` writes `TRUE`/`FALSE`), and arrays and JSON items stay errors.
>
> | Expression | Before | After |
> |---|---|---|
> | `[2.5::Currency].join("-")` | `Error: join method cannot convert value to string` | `"2.5000"` |
> | `[1::Currency, -2.25::Currency].join("; ")` | `Error` | `"1.0000; -2.2500"` |
> | `[1, 2.5::Currency, NULL].join(",")` | `Error` | `"1,2.5000,"` (= `JOIN([1, 2.5::Currency, NULL], ",")`) |
> | `[1, NULL, 2].join("-")`, `[NULL].join()` | `Error` | `"1--2"`, `""` |
> | `[TRUE, 1].join()` | `"true,1"` | unchanged (`JOIN` gives `"TRUE,1"`) |
> | `[[1,2],3].join()` | `Error: join method cannot convert value to string` | unchanged |
> | `[].join()`, `[1.5, "a"].join()`, `[DATE(2026,1,1)].join()` | `""`, `"1.5,a"`, `"2026-01-01T00:00:00Z"` | unchanged |
>
> **A4. `LEN` and `FIND` on a date stay errors.** No change: `LEN(DATE(2026,1,1))` is
> `Error: LEN expects text`; `FIND("2", DATE(2026,1,1))` is
> `Error: FIND expects within_text as text`. Convert first: `LEN(DATE(2026,1,1)::String)` is `20`.
>
> **A5. `SUMIF` and `AVERAGEIF` keep Currency.** The rule of decision 3, applied to the values
> they actually sum or average: the matched items' values (from the sum/average range when given,
> otherwise the item), counting only Numbers and Currency. If any of those is Currency, the result
> is Currency; otherwise a Number. A Currency that is not summed (not matched, or in the criteria
> range while a sum range is given) does not count. The lambda criterion works the same way. No
> summed value keeps the old result: `SUMIF` gives `Number(0)`, `AVERAGEIF` the `#DIV/0!` error.
> `COUNTIF` is unchanged (always a Number). Results are not rounded.
>
> | Expression | Before | After |
> |---|---|---|
> | `SUMIF([1,2::Currency,3], ">1")` | `Number(5)` | `Currency(5)` |
> | `SUMIF([1::Currency,2::Currency], ">0")` | `Number(3)` | `Currency(3)` |
> | `SUMIF([1::Currency,2,3], ">1")` (the Currency is not matched) | `Number(5)` | unchanged |
> | `SUMIF([1,2,3], ">1")` | `Number(5)` | unchanged |
> | `SUMIF([1,2,3], ">1", [10, 20::Currency, 30])` | `Number(50)` | `Currency(50)` |
> | `SUMIF([1,2,3], ">1", [10::Currency, 20, 30])` (10 is not summed) | `Number(50)` | unchanged |
> | `SUMIF([1::Currency,2,3], ">0", [10, 20, 30])` (Currency only in the criteria range) | `Number(60)` | unchanged |
> | `SUMIF([1, 2], :x > 1, [10, 20::Currency])` (lambda criterion) | `Number(20)` | `Currency(20)` |
> | `SUMIF([1::Currency, "a", 2], ">0")` | `Number(3)` | `Currency(3)` (text ignored) |
> | `SUMIF([1::Currency], ">5")`, `SUMIF([], ">0")` | `Number(0)` | unchanged |
> | `AVERAGEIF([1,2::Currency,3], ">1")` | `Number(2.5)` | `Currency(2.5)` |
> | `AVERAGEIF([1::Currency,2::Currency], ">0")` | `Number(1.5)` | `Currency(1.5)` |
> | `AVERAGEIF([1::Currency,2,3], ">1")` | `Number(2.5)` | unchanged |
> | `AVERAGEIF([1,2,3], ">1", [10::Currency, 20, 30])`, `[10, 20::Currency, 30]` | `Number(25)` | unchanged, `Currency(25)` |
> | `AVERAGEIF([1,2,3], ">5", [1::Currency,2,3])` | `Error: AVERAGEIF: no matching numbers (#DIV/0!)` | unchanged |
> | `COUNTIF([1::Currency,2::Currency], ">1")` | `Number(1)` | unchanged |
>
> This replaces "`SUMIF`/`AVERAGEIF` … are not part of this decision" in decision 3; the
> statistical builtins (`MEDIAN`, `STDEV.*`, …) and the methods are still not part of it.

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
