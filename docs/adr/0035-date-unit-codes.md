# 0035. Date unit codes: VBA codes in `DATEADD`/`DATEDIFF`, Excel codes in `DATEDIF`

- Status: Accepted
- Date: 2026-11-22
- Deciders: Maintainer (B-15, BL-074; recorded for audit LANG-4)
- Related: ADR-0018 (Excel is the contract for builtins), BL-074 (PR #103),
  `docs/reference/excel-compat.md` (`DATEADD`, `DATEDIFF`, `DATEDIF` rows, B-15),
  `docs/guide/migration-0.7.md` ("Date units follow Excel"), `src/runtime/datetime.rs`

## Context

In 0.6.3 `DATEADD` and `DATEDIFF` took Skillet's own unit names. The single letters were
abbreviations of the long words: `"m"` was minutes, `"y"` years and `"w"` weeks. Excel has no
worksheet function by either name; the closest are VBA's `DateAdd` and `DateDiff`, which take
the codes `yyyy q m y d w ww h n s`. There `"m"` is months, `"n"` minutes, `"y"` day of year and
`"w"` weekday; in `DateAdd` both `"y"` and `"w"` add one day. Users who know Excel expect
`"m"` to be months (audit B-15).

Excel's worksheet function `DATEDIF` takes other codes: `"y"`, `"m"`, `"d"` for complete years,
months and days, and `"md"`, `"ym"`, `"yd"` for the remainders.

BL-074 (#103) implemented the maintainer's choice, but the decision was recorded only as a
backlog line and a changelog entry. Since then the same letter means different things in two
functions whose names differ by one letter (measured on develop, `sk`):

| Expression | Result |
|---|---|
| `DATEDIF(DATE(2026,1,1), DATE(2027,1,1), "y")` | `1` (years) |
| `DATEDIFF(DATE(2026,1,1), DATE(2027,1,1), "y")` | `365` (days) |
| `DATEDIF(DATE(2026,1,31), DATE(2026,2,1), "m")` | `0` (complete months) |
| `DATEDIFF(DATE(2026,1,31), DATE(2026,2,1), "m")` | `1` (month boundaries) |
| `DATEADD(DATE(2026,1,1), 1, "w")`, `…, "weeks")` | 2026-01-02, 2026-01-08 |
| `DATEDIFF(DATE(2026,1,3), DATE(2026,1,4), "weeks")`, `…, "ww")` | `0`, `1` |

## Decision

1. **`DATEADD` and `DATEDIFF` take VBA's `DateAdd`/`DateDiff` codes**, in any case:
   `yyyy` years, `q` quarters, `m` months, `y` and `d` days, `w` weekday (adds days, counts
   whole weeks), `ww` calendar weeks (adds weeks, counts Sundays), `h` hours, `n` minutes,
   `s` seconds.
2. **The long words stay** and keep their 0.6 meaning: `years`, `months`, `weeks`, `days`,
   `hours`, `minutes`, `seconds` (and the singular forms). `"weeks"` adds and counts whole weeks.
3. **`DATEADD` clamps to the end of the month** for months, quarters and years, as VBA does
   (`DATEADD(DATE(2024,1,31), 1, "m")` is 2024-02-29). A result outside the supported range is
   `#NUM!`.
4. **`DATEDIFF` counts boundaries** for `yyyy`, `q`, `m` and `ww`, as VBA does, and whole elapsed
   units for `d`, `y`, `w`, `h`, `n`, `s` (the same for dates at midnight).
5. **`DATEDIF` takes Excel's worksheet codes** `y m d md ym yd`, in any case, with Excel's rules
   for the remainders. Start after end, or any other code (including the VBA codes such as
   `"yyyy"`), is `#NUM!`, as in Excel.
6. An unknown unit in `DATEADD`/`DATEDIFF` is `#VALUE!` and lists the accepted units.

Each function follows the Excel-family function it is named after, so a formula copied from VBA
or from a worksheet gives the same answer.

## Consequences

- Three codes changed meaning silently between 0.6 and 0.7: `"m"`, `"y"`, and `"w"` in `DATEADD`.
  The migration guide lists them under "Check these first" with the code that keeps the old
  result (`"n"`, `"yyyy"`, `"ww"`, or the long words).
- `"y"` and `"w"` in `DATEADD` are a plain day, which almost nobody types on purpose. Their main
  effect is that an old `"y"` meaning years now gives a result that is off by about 365 days
  without an error. The docs recommend the long words for new formulas.
- The same letter means different things in `DATEDIF` and `DATEDIFF`. The reference rows for both
  functions say so, and `DATEDIF` rejects the VBA-only codes instead of guessing.

## Alternatives considered

- Make `"y"` and `"w"` a `#VALUE!` error in `DATEADD`/`DATEDIFF` that suggests `"d"`, `"yyyy"` or
  `"ww"` (audit LANG-4). Loud instead of silent, but breaks VBA parity; not chosen by the
  maintainer.
- Keep the 0.6 abbreviations (`"m"` minutes). Surprises every Excel and VBA user; rejected in B-15.
- Excel's `DATEDIF` codes in `DATEADD`/`DATEDIFF` too. `DATEDIF` has no codes for quarters, weeks,
  hours, minutes or seconds, so they would need Skillet-only codes; rejected.
