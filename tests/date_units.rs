//! Date unit codes follow Excel (B-15, decided 2026-11-21).
//!
//! `DATEADD` and `DATEDIFF` read VBA `DateAdd`/`DateDiff` codes, so `"m"` is months and `"n"`
//! minutes; `DATEDIF` is Excel's worksheet `DATEDIF`. Examples marked "Microsoft" are from the
//! VBA `DateAdd`/`DateDiff` and Excel `DATEDIF` reference pages.

use skillet::{evaluate, Value};

fn eval(expr: &str) -> Value {
    evaluate(expr).unwrap_or_else(|e| panic!("{expr}: {}", e.message))
}

fn num(expr: &str) -> f64 {
    match eval(expr) {
        Value::Number(n) => n,
        other => panic!("{expr}: expected a number, got {other:?}"),
    }
}

fn err(expr: &str) -> String {
    match evaluate(expr) {
        Err(e) => e.message,
        Ok(v) => panic!("{expr}: expected an error, got {v:?}"),
    }
}

/// `DATEADD(DATE(2026, 1, 31) + 0.5, amount, unit)` equals `want` (a Skillet expression).
fn adds(amount: &str, unit: &str, want: &str) {
    let expr = format!("=DATEADD(DATE(2026, 1, 31) + 0.5, {amount}, \"{unit}\")");
    assert_eq!(eval(&expr), eval(want), "{expr}");
}

#[test]
fn dateadd_reads_vba_dateadd_codes() {
    // Base: 2026-01-31 12:00 UTC.
    adds("1", "yyyy", "=DATE(2027, 1, 31) + 0.5");
    adds("1", "q", "=DATE(2026, 4, 30) + 0.5");
    adds("1", "m", "=DATE(2026, 2, 28) + 0.5");
    adds("1", "y", "=DATE(2026, 2, 1) + 0.5");
    adds("1", "d", "=DATE(2026, 2, 1) + 0.5");
    adds("1", "w", "=DATE(2026, 2, 1) + 0.5");
    adds("1", "ww", "=DATE(2026, 2, 7) + 0.5");
    adds("1", "h", "=DATE(2026, 1, 31) + 0.5 + TIME(1, 0, 0)");
    adds("1", "n", "=DATE(2026, 1, 31) + 0.5 + TIME(0, 1, 0)");
    adds("1", "s", "=DATE(2026, 1, 31) + 0.5 + TIME(0, 0, 1)");
}

#[test]
fn dateadd_m_is_months_and_n_is_minutes() {
    adds("1", "m", "=DATE(2026, 2, 28) + 0.5");
    adds("1", "n", "=DATE(2026, 1, 31) + 0.5 + TIME(0, 1, 0)");
    adds("-2", "m", "=DATE(2025, 11, 30) + 0.5");
}

#[test]
fn dateadd_codes_ignore_case() {
    for unit in ["M", "YYYY", "Q", "WW", "N", "Months", "MINUTES"] {
        let lower = unit.to_lowercase();
        let a = format!("=DATEADD(DATE(2026, 1, 31), 3, \"{unit}\")");
        let b = format!("=DATEADD(DATE(2026, 1, 31), 3, \"{lower}\")");
        assert_eq!(eval(&a), eval(&b), "{unit}");
    }
}

#[test]
fn dateadd_keeps_skillet_long_words() {
    adds("1", "years", "=DATE(2027, 1, 31) + 0.5");
    adds("1", "year", "=DATE(2027, 1, 31) + 0.5");
    adds("1", "months", "=DATE(2026, 2, 28) + 0.5");
    adds("1", "weeks", "=DATE(2026, 2, 7) + 0.5");
    adds("2", "days", "=DATE(2026, 2, 2) + 0.5");
    adds("1", "hours", "=DATE(2026, 1, 31) + 0.5 + TIME(1, 0, 0)");
    adds("1", "minutes", "=DATE(2026, 1, 31) + 0.5 + TIME(0, 1, 0)");
    adds("1", "seconds", "=DATE(2026, 1, 31) + 0.5 + TIME(0, 0, 1)");
}

#[test]
fn dateadd_month_end_clamps_like_vba() {
    // Microsoft: DateAdd("m", 1, "31-Jan-95") returns 28-Feb-95 (29 in leap years).
    assert_eq!(
        eval("=DATEADD(DATE(1995, 1, 31), 1, \"m\")"),
        eval("=DATE(1995, 2, 28)")
    );
    assert_eq!(
        eval("=DATEADD(DATE(2024, 1, 31), 1, \"m\")"),
        eval("=DATE(2024, 2, 29)")
    );
    assert_eq!(
        eval("=DATEADD(DATE(2026, 3, 31), 1, \"m\")"),
        eval("=DATE(2026, 4, 30)")
    );
    assert_eq!(
        eval("=DATEADD(DATE(2024, 2, 29), 1, \"yyyy\")"),
        eval("=DATE(2025, 2, 28)")
    );
}

#[test]
fn dateadd_unknown_unit_is_value_error() {
    for unit in ["eons", "mm", "", "md"] {
        let msg = err(&format!("=DATEADD(DATE(2026, 1, 1), 1, \"{unit}\")"));
        assert!(msg.ends_with("(#VALUE!)"), "{unit}: {msg}");
    }
}

#[test]
fn dateadd_out_of_range_is_an_error_not_a_panic() {
    for expr in [
        "=DATEADD(DATE(2026, 1, 1), 1e15, \"m\")",
        "=DATEADD(DATE(2026, 1, 1), 1e18, \"d\")",
        "=DATEADD(DATE(2026, 1, 1), -1e18, \"s\")",
        "=DATEADD(DATE(2026, 1, 1), 1e300, \"yyyy\")",
    ] {
        assert!(err(expr).ends_with("(#NUM!)"), "{expr}");
    }
}

#[test]
fn datediff_reads_vba_datediff_codes() {
    // Microsoft: Dec 31 to Jan 1 is 1 for "yyyy", though only a day has elapsed.
    assert_eq!(
        num("=DATEDIFF(DATE(2025, 12, 31), DATE(2026, 1, 1), \"yyyy\")"),
        1.0
    );
    assert_eq!(
        num("=DATEDIFF(DATE(2026, 3, 31), DATE(2026, 4, 1), \"q\")"),
        1.0
    );
    assert_eq!(
        num("=DATEDIFF(DATE(2026, 1, 31), DATE(2026, 2, 1), \"m\")"),
        1.0
    );
    assert_eq!(
        num("=DATEDIFF(DATE(2026, 1, 1), DATE(2026, 3, 1), \"y\")"),
        59.0
    );
    assert_eq!(
        num("=DATEDIFF(DATE(2026, 1, 1), DATE(2026, 3, 1), \"d\")"),
        59.0
    );
    assert_eq!(
        num("=DATEDIFF(DATE(2026, 1, 1), DATE(2026, 1, 1) + 0.5, \"h\")"),
        12.0
    );
    assert_eq!(
        num("=DATEDIFF(DATE(2026, 1, 1), DATE(2026, 1, 1) + 0.5, \"n\")"),
        720.0
    );
    assert_eq!(
        num("=DATEDIFF(DATE(2026, 1, 1), DATE(2026, 1, 1) + 0.5, \"s\")"),
        43200.0
    );
}

#[test]
fn datediff_w_counts_weeks_and_ww_counts_sundays() {
    // Microsoft: "w" counts weeks from date1's weekday; "ww" counts the Sundays after date1 up
    // to and including date2. 2026-01-03 is a Saturday, 2026-01-04 a Sunday.
    assert_eq!(
        num("=DATEDIFF(DATE(2026, 1, 3), DATE(2026, 1, 4), \"w\")"),
        0.0
    );
    assert_eq!(
        num("=DATEDIFF(DATE(2026, 1, 3), DATE(2026, 1, 4), \"ww\")"),
        1.0
    );
    assert_eq!(
        num("=DATEDIFF(DATE(2026, 1, 4), DATE(2026, 1, 10), \"ww\")"),
        0.0
    );
    assert_eq!(
        num("=DATEDIFF(DATE(2026, 1, 4), DATE(2026, 1, 18), \"ww\")"),
        2.0
    );
    assert_eq!(
        num("=DATEDIFF(DATE(2026, 1, 1), DATE(2026, 1, 15), \"w\")"),
        2.0
    );
    assert_eq!(
        num("=DATEDIFF(DATE(2026, 1, 18), DATE(2026, 1, 4), \"ww\")"),
        -2.0
    );
}

#[test]
fn datediff_m_is_months_and_n_is_minutes() {
    assert_eq!(
        num("=DATEDIFF(DATE(2026, 1, 1), DATE(2026, 3, 1), \"m\")"),
        2.0
    );
    assert_eq!(
        num("=DATEDIFF(DATE(2026, 1, 1), DATE(2026, 1, 1) + TIME(0, 5, 0), \"n\")"),
        5.0
    );
    assert_eq!(
        num("=DATEDIFF(DATE(2026, 1, 1), DATE(2026, 3, 1), \"MONTHS\")"),
        2.0
    );
    assert_eq!(
        num("=DATEDIFF(DATE(2026, 1, 1), DATE(2026, 1, 1) + TIME(0, 5, 0), \"Minutes\")"),
        5.0
    );
}

#[test]
fn datediff_unknown_unit_is_value_error() {
    let msg = err("=DATEDIFF(DATE(2026, 1, 1), DATE(2026, 2, 1), \"ym\")");
    assert!(msg.ends_with("(#VALUE!)"), "{msg}");
}

#[test]
fn datedif_microsoft_examples() {
    // Microsoft DATEDIF examples.
    assert_eq!(
        num("=DATEDIF(DATE(2001, 1, 1), DATE(2003, 1, 1), \"Y\")"),
        2.0
    );
    assert_eq!(
        num("=DATEDIF(DATE(2001, 6, 1), DATE(2002, 8, 15), \"D\")"),
        440.0
    );
    assert_eq!(
        num("=DATEDIF(DATE(2001, 6, 1), DATE(2002, 8, 15), \"YD\")"),
        75.0
    );
    // Same dates, the other units: 1 year 2 months 14 days.
    assert_eq!(
        num("=DATEDIF(DATE(2001, 6, 1), DATE(2002, 8, 15), \"m\")"),
        14.0
    );
    assert_eq!(
        num("=DATEDIF(DATE(2001, 6, 1), DATE(2002, 8, 15), \"ym\")"),
        2.0
    );
    assert_eq!(
        num("=DATEDIF(DATE(2001, 6, 1), DATE(2002, 8, 15), \"md\")"),
        14.0
    );
}

#[test]
fn datedif_counts_complete_units() {
    assert_eq!(
        num("=DATEDIF(DATE(2026, 1, 31), DATE(2026, 2, 1), \"m\")"),
        0.0
    );
    assert_eq!(
        num("=DATEDIF(DATE(2025, 12, 31), DATE(2026, 1, 1), \"y\")"),
        0.0
    );
    assert_eq!(
        num("=DATEDIF(DATE(2024, 2, 29), DATE(2025, 2, 28), \"y\")"),
        0.0
    );
    assert_eq!(
        num("=DATEDIF(DATE(2024, 2, 29), DATE(2025, 3, 1), \"y\")"),
        1.0
    );
    assert_eq!(
        num("=DATEDIF(DATE(2026, 1, 1), DATE(2026, 1, 1), \"d\")"),
        0.0
    );
    // Times are ignored, as Excel truncates the serial numbers.
    assert_eq!(
        num("=DATEDIF(DATE(2026, 1, 1) + 0.9, DATE(2026, 1, 2) + 0.1, \"d\")"),
        1.0
    );
}

#[test]
fn datedif_md_and_yd_follow_excel() {
    // Excel's "md" is end - DATE(YEAR(end), MONTH(end) - 1, DAY(start)), which Microsoft warns
    // can be negative: here DATE(2011, 2, 31) rolls over to 2011-03-03.
    assert_eq!(
        num("=DATEDIF(DATE(2011, 1, 31), DATE(2011, 3, 1), \"md\")"),
        -2.0
    );
    assert_eq!(
        num("=DATEDIF(DATE(2026, 1, 20), DATE(2026, 3, 5), \"md\")"),
        13.0
    );
    assert_eq!(
        num("=DATEDIF(DATE(2025, 11, 15), DATE(2026, 2, 10), \"yd\")"),
        87.0
    );
}

#[test]
fn datedif_errors_follow_excel() {
    let msg = err("=DATEDIF(DATE(2026, 2, 1), DATE(2026, 1, 1), \"d\")");
    assert!(msg.ends_with("(#NUM!)"), "{msg}");
    for unit in ["yyyy", "q", "n", "days", ""] {
        let msg = err(&format!(
            "=DATEDIF(DATE(2026, 1, 1), DATE(2026, 2, 1), \"{unit}\")"
        ));
        assert!(msg.ends_with("(#NUM!)"), "{unit}: {msg}");
    }
    assert_eq!(
        err("=DATEDIF(DATE(2026, 1, 1), DATE(2026, 2, 1))"),
        "DATEDIF expects start_date, end_date, unit"
    );
}
