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

> **Amendment 2 (maintainer, 2026-11-22, BL-090; audit LANG-5, LANG-6):** dates in the
> aggregates, in sorting and in criteria. The maintainer decided the direction on 2026-11-22:
> `MAX`/`MIN` and every sort accept dates, `SUM` of dates is an error, and criteria read ISO date
> text and compare dates by instant. The details below (mixes, the other aggregates, messages,
> text cells) are the Language Designer's and wait for acceptance. This amendment sits here, not
> in ADR-0027, because it is about this ADR's two subjects: what the aggregates count, and the
> ISO text that decision 2 lets `::DateTime` read. ADR-0027 is about the arithmetic operators,
> which do not change.
>
> **Problem.** `MAX(DATE(2026,1,1), DATE(2027,1,1))` is `0` and `SUM(DATE(2026,1,1))` is `0`: the
> aggregates skip dates as if they were text, so "latest date" silently gives a number.
> `SORT` and `.sort()` reject dates although `<` orders them (ADR-0023 §3).
> `COUNTIF(r, ">" & DATE(2026,1,1))`, the usual Excel idiom, builds the text
> `">2026-01-01T00:00:00Z"`, which compares as text and never matches a date.
>
> **The rule in one line.** Functions that pick or order values (`MIN`, `MAX`, `SORT`, `.sort()`,
> `.min()`, `.max()`, `BETWEEN`, criteria) accept dates and compare them by instant; functions
> that do arithmetic on values (`SUM`, `AVERAGE`, `MEDIAN`, …) reject them with `#VALUE!`. A date
> and a number are never compared (ADR-0023 §3): where both meet, it is an error. Skillet has a
> DateTime type and no date serial numbers, so this differs from Excel on purpose: in Excel a date
> is a number, `SUM` of dates is a (meaningless) number and dates mix freely with numbers.
>
> **B1. `MIN` and `MAX` return a date for dates.** They count Numbers and DateTimes, arrays
> flattened at any depth. Text, booleans, `NULL` and JSON are still ignored.
> - Only DateTimes counted (at least one): the earliest (`MIN`) or latest (`MAX`) DateTime.
> - DateTimes and Numbers both counted: `Error: MAX cannot compare dates with numbers (#VALUE!)`
>   (`MIN …` for `MIN`). Ignored values never cause it.
> - Nothing counted: `0`, as now.
>
> **B2. The number aggregates reject dates.** `SUM`, `PRODUCT` (`MULTIPLY`), `AVERAGE` (`AVG`),
> `MEDIAN`, `MODE.SNGL`, `STDEV.P`, `STDEV.S`, `VAR.P`, `VAR.S`, `PERCENTILE.INC` and
> `QUARTILE.INC` (each with its aliases) return `Error: <NAME> does not accept dates (#VALUE!)` when
> a DateTime is among their data, arrays flattened. `<NAME>` is the one in the left column of this
> list, whichever alias was called. The check is made while the data is collected, so it comes
> before the "no numbers" errors (`AVERAGE(DATE(2026,1,1))` is this error, not `#DIV/0!`), and for
> `PERCENTILE.INC`/`QUARTILE.INC` after the `k`/`quart` argument checks of today. `COUNT` still
> counts dates and `COUNTA` is unchanged. A mean date is still one line away:
> `DATE(1970,1,1) + AVERAGE(MAP(:dates, :x - DATE(1970,1,1)))`.
>
> **B3. `SUMIF` and `AVERAGEIF` reject a date they would sum.** A matched value that is summed or
> averaged (from the sum range when given, otherwise the item) and is a DateTime gives
> `Error: SUMIF does not accept dates (#VALUE!)` (`AVERAGEIF`, also when called as `AVGIF`).
> Dates that are not summed (not matched, or in the criteria range while a sum range is given) do
> not count. `COUNTIF` counts dates as before. This makes `SUMIF(:dates, ">2026-01-01T00:00:00Z")`
> an error instead of `0`.
>
> **B4. `SORT` orders dates.** A DateTime takes the number place of Excel's type order: numbers
> and dates first, then text, then booleans. Two DateTimes compare by instant; equal ones keep
> their order, in both directions. An array that holds both a Number and a DateTime is
> `Error: SORT cannot compare dates with numbers (#VALUE!)`. `NULL`, arrays and JSON stay errors,
> and the message names dates: `SORT expects numbers, dates, text or booleans`.
>
> **B5. `.sort()`, `.min()` and `.max()` accept an array of dates.** As in Ruby, an array of Times
> sorts and a mix with numbers does not. An array of only Numbers works as now; an array of only
> DateTimes gives the DateTimes sorted (`.sort("DESC")` descending) or the earliest/latest
> DateTime. Anything else (a mix, text, `NULL`, …) is an error:
> `sort expects an array of numbers or of dates`, `min method expects an array of numbers or of
> dates`, `max method expects an array of numbers or of dates`. The empty array keeps its
> results: `[].sort()` is `[]`, `[].min()` and `[].max()` are `NULL`. `.sum()` and `.avg()` are
> unchanged (dates are an error, with today's messages).
>
> **B6. `BETWEEN` accepts three dates.** `BETWEEN(min, max, value)` with three DateTimes is
> `min <= value <= max` by instant. Any other mix keeps today's number rule and messages
> (`BETWEEN(DATE(2026,1,1), 10, 5)` is `Error: BETWEEN min must be a number`). The `.between()`
> number method is unchanged.
>
> **B7. Equality is already by instant (no change).** `==`, `!=`, `UNIQUE`, `.unique()`,
> `CONTAINS`, `IN` and `.includes()` compare two DateTimes by their seconds, whatever made them:
> `DATE(2026,1,1) == "2026-01-01T00:00:00Z"::DateTime` and `== 1767225600::DateTime` are `true`.
> A DateTime never equals another type: `DATE(2026,1,1) == "2026-01-01T00:00:00Z"` and
> `== 1767225600` stay `false` (ADR-0023 §3). Whether the date builtins (`YEAR`, `DATEADD`, …)
> read ISO text is not part of this amendment.
>
> **B8. Criteria compare dates.** `COUNTIF`, `SUMIF` and `AVERAGEIF` get a date operand.
> - A static DateTime criterion means `=` that instant: `COUNTIF(r, DATE(2026,1,1))`. Today it is
>   `Error: COUNTIF condition must be TRUE, FALSE or a number, got datetime (#VALUE!)`.
> - A text criterion is read as today (optional operator, then: empty is blank, a number, `TRUE`/
>   `FALSE`), with one new step before "anything else is text": an operand that is exactly the
>   ISO text of decision 2 is a date. The test is the one `::DateTime` makes, minus integer text
>   (which is already a number operand, so `">1767225600"` stays a number): sign and long years
>   allowed; no date-only text, no offsets, no fractions, no spaces, uppercase `T`/`Z`, no
>   rollover. So `">" & DATE(2026,1,1)` and `"=" & DATE(2026,1,1)` work, and every operator
>   (`=`, none, `<>`, `<`, `<=`, `>`, `>=`) applies. Text with `*` or `?` is never a date; it
>   stays a wildcard pattern.
> - Date-only text (`">2026-01-01"`) is not a date, as for `::DateTime`, so it stays a text
>   operand and matches no date. Use `">" & DATE(2026,1,1)`. If date-only text is ever accepted,
>   the cast and criteria change together (one parser, `parse_datetime_text`).
> - A date operand matches: a DateTime item, compared by instant; a text item that is exactly the
>   ISO form (not integer text), read as its instant, for every operator; nothing else, except
>   that `<>` matches every item that is not equal (numbers, other text, booleans, `NULL`), as
>   `"<>5"` does. Text items are read because data from JSON holds dates as text, and today's
>   text comparison already matched ISO text of the same shape; it now compares the instant.
> - The other operands do not change: a number operand never matches a date (except `<>`), a text
>   or pattern operand never matches a date (except `<>`), and a date is not blank.
> - Lambda criteria are unchanged (`COUNTIF(r, :x > DATE(2026,1,1))` already works).
>
> | Expression | Before | After |
> |---|---|---|
> | `MAX(DATE(2026,1,1), DATE(2027,1,1))` | `0` | `DateTime(1798761600)` (2027-01-01) |
> | `MIN([DATE(2026,1,1), DATE(2025,6,1)])`, `MAX([DATE(2026,1,1), [DATE(2027,3,1)]])` | `0`, `0` | 2025-06-01, 2027-03-01 |
> | `MAX(DATE(2026,1,1), NULL)`, `MAX(DATE(2026,1,1), "x", TRUE)`, `MAX(DATE(2026,1,1), [])` | `0` | 2026-01-01 (ignored values do not count) |
> | `MAX(DATE(2026,1,1), 5)`, `MIN(DATE(2026,1,1), 5)` | `5` | `Error: MAX cannot compare dates with numbers (#VALUE!)`, `MIN …` |
> | `MAX()`, `MAX([])`, `MAX(NULL)`, `MAX(1, 5)` | `0`, `0`, `0`, `5` | unchanged |
> | `SUM(DATE(2026,1,1))`, `SUM(DATE(2026,1,1), 1)`, `SUM([1, DATE(2026,1,1)])` | `0`, `1`, `1` | `Error: SUM does not accept dates (#VALUE!)` |
> | `PRODUCT(DATE(2026,1,1))`, `MULTIPLY(…)` | `1` | `Error: PRODUCT does not accept dates (#VALUE!)` |
> | `AVERAGE(DATE(2026,1,1), DATE(2026,1,3))`, `AVG(DATE(2026,1,1))` | `Error: AVERAGE of no numbers (#DIV/0!)` | `Error: AVERAGE does not accept dates (#VALUE!)` |
> | `MEDIAN(DATE(2026,1,1), DATE(2026,1,3))`, `MEDIAN(1, DATE(2026,1,3))` | `Error: MEDIAN of no numbers (#NUM!)`, `1` | `Error: MEDIAN does not accept dates (#VALUE!)` |
> | `STDEV.P(1, 2, DATE(2026,1,3))`, `STDEV(DATE(2026,1,1), 1)` | `0.5`, `Error: STDEV needs at least two numbers` | `Error: STDEV.P does not accept dates (#VALUE!)`, `Error: STDEV.S …` |
> | `PERCENTILE.INC([DATE(2026,1,1)], 0.5)` | `Error: PERCENTILE.INC of no numbers (#NUM!)` | `Error: PERCENTILE.INC does not accept dates (#VALUE!)` |
> | `COUNT(DATE(2026,1,1), 1, "a")` | `2` | unchanged |
> | `SORT([DATE(2027,1,1), DATE(2026,1,1)])`, `… , -1)` | `Error: SORT expects numbers, text or booleans` | `[2026-01-01, 2027-01-01]`, `[2027-01-01, 2026-01-01]` |
> | `SORT([DATE(2026,1,1), "a", TRUE])` | `Error` | `[2026-01-01, "a", TRUE]` |
> | `SORT([DATE(2026,1,1), 5])` | `Error: SORT expects numbers, text or booleans` | `Error: SORT cannot compare dates with numbers (#VALUE!)` |
> | `SORT([1, NULL])` | `Error: SORT expects numbers, text or booleans` | `Error: SORT expects numbers, dates, text or booleans` |
> | `[DATE(2027,1,1), DATE(2026,1,1)].sort()`, `.sort("DESC")` | `Error: sort expects numeric array` | `[2026-01-01, 2027-01-01]`, `[2027-01-01, 2026-01-01]` |
> | `[DATE(2027,1,1), DATE(2026,1,1)].max()`, `.min()` | `Error: max method expects numeric array`, `min …` | 2027-01-01, 2026-01-01 |
> | `[DATE(2027,1,1), 1].max()`, `["b", "a"].sort()` | `Error: max method expects numeric array`, `Error: sort expects numeric array` | `Error: max method expects an array of numbers or of dates`, `Error: sort expects an array of numbers or of dates` |
> | `[DATE(2026,1,1)].sum()`, `.avg()` | `Error: sum method expects numeric array`, `avg …` | unchanged |
> | `BETWEEN(DATE(2026,1,1), DATE(2027,1,1), DATE(2026,6,1))` | `Error: BETWEEN min must be a number` | `true` |
> | `UNIQUE([DATE(2026,1,1), DATE(2026,1,1), DATE(2027,1,1)])`, `CONTAINS([DATE(2026,1,1)], DATE(2026,1,1))` | two dates, `true` | unchanged |
> | `COUNTIF([DATE(2026,1,1), DATE(2027,1,1)], ">" & DATE(2026,6,1))` | `0` | `1` |
> | `COUNTIF(…same…, ">=2026-01-01T00:00:00Z")`, `"<2027-01-01T00:00:00Z"`, `"<=2027-01-01T00:00:00Z"` | `0`, `0`, `0` | `2`, `1`, `2` |
> | `COUNTIF(…same…, "2026-01-01T00:00:00Z")`, `"=" & DATE(2026,1,1)`, `DATE(2026,1,1)` | `0`, `0`, `Error: COUNTIF condition must be …, got datetime (#VALUE!)` | `1`, `1`, `1` |
> | `COUNTIF(…same…, "<>2026-01-01T00:00:00Z")` | `2` | `1` |
> | `COUNTIF(…same…, ">2026-06-01")` (date-only) | `0` | unchanged (text operand) |
> | `COUNTIF([DATE(2026,1,1), 5, "a", NULL], ">2025-01-01T00:00:00Z")` | `1` (`"a"` as text) | `1` (the date) |
> | `COUNTIF([DATE(2026,1,1), 5, "a", NULL], "<>2026-01-01T00:00:00Z")` | `4` | `3` |
> | `COUNTIF([DATE(2026,1,1), 5], ">0")`, `">1767225599"` | `1`, `0` | unchanged (a number never matches a date) |
> | `COUNTIF(["2026-02-01T00:00:00Z", "2025-02-01T00:00:00Z"], ">2026-01-01T00:00:00Z")` | `1` (as text) | `1` (by instant) |
> | `COUNTIF(["2026-01-01T00:00:00Z"], DATE(2026,1,1))` | `Error` | `1` |
> | `COUNTIF(["2026-01-01t00:00:00z"], "=2026-01-01T00:00:00Z")` | `1` (text, ignoring case) | `0` (not ISO text, so not a date) |
> | `COUNTIF(["1767225600"], DATE(2026,1,1))` | `Error` | `0` (integer text is not a date) |
> | `COUNTIF([DATE(2026,1,1)], "2026-01-01T00:00:00*")`, `COUNTIF([DATE(2026,1,1)], ">2026-02-30T00:00:00Z")` | `0`, `0` | unchanged (a pattern; rollover is text) |
> | `COUNTIF([DATE(2026,1,1)], ">+10000-01-01T00:00:00Z")` | `0` | unchanged (now by instant) |
> | `SUMIF([DATE(2026,1,1), DATE(2027,1,1)], ">2026-06-01T00:00:00Z", [10, 20])` | `0` | `20` |
> | `AVERAGEIF([DATE(2026,1,1), DATE(2027,1,1)], ">2025-06-01T00:00:00Z", [10, 20])` | `Error: AVERAGEIF: no matching numbers (#DIV/0!)` | `15` |
> | `SUMIF([DATE(2026,1,1), DATE(2027,1,1)], ">2026-06-01T00:00:00Z")` | `0` | `Error: SUMIF does not accept dates (#VALUE!)` |
> | `SUMIF([1, 2], ">0", [DATE(2026,1,1), 5])`, `SUMIF([1, 2], ">1", [DATE(2026,1,1), 5])` | `5`, `5` | `Error: SUMIF does not accept dates (#VALUE!)`, `5` (the date is not matched) |
> | `COUNTIF([DATE(2026,1,1), DATE(2027,1,1)], :x > DATE(2026,6,1))` | `1` | unchanged |
>
> Breaking in 0.7.0: results that used to be a silent `0` (or a number that skipped the dates)
> become a date or an error, and three error messages change (`SORT`, `.sort()`, `.min()`/
> `.max()`).

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
