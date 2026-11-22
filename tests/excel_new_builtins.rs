//! BL-054: Excel-named builtins added for excel-compat D-9 (`LEN`, `TODAY`, `PPMT`, `PV`, `NPV`,
//! `TEXTJOIN`, `PERCENTILE`, `QUARTILE`, `MODE`), Excel `FIND` on text (D-6), and Excel `DATE`
//! rollover plus date arithmetic in days (D-5). Expected values are Excel's: most come from the
//! examples in Microsoft's function reference; the others are worked out from Excel's documented
//! formula in the comment next to them.

use skillet::{evaluate, Value};

fn num(expr: &str) -> f64 {
    match evaluate(expr) {
        Ok(Value::Number(n)) => n,
        other => panic!("{expr}: expected a number, got {other:?}"),
    }
}

fn close(expr: &str, want: f64) {
    let got = num(expr);
    assert!((got - want).abs() < 0.005, "{expr}: got {got}, want {want}");
}

fn text(expr: &str) -> String {
    match evaluate(expr) {
        Ok(Value::String(s)) => s,
        other => panic!("{expr}: expected a string, got {other:?}"),
    }
}

fn date(expr: &str) -> i64 {
    match evaluate(expr) {
        Ok(Value::DateTime(ts)) => ts,
        other => panic!("{expr}: expected a DateTime, got {other:?}"),
    }
}

fn err(expr: &str) -> String {
    match evaluate(expr) {
        Err(e) => e.message,
        Ok(v) => panic!("{expr}: expected an error, got {v:?}"),
    }
}

/// Error message of `expr` evaluated with `:nan` = NaN (NaN from data, not from an operator).
fn err_nan(expr: &str) -> String {
    let vars = std::collections::HashMap::from([("nan".to_string(), Value::Number(f64::NAN))]);
    match skillet::evaluate_with(expr, &vars) {
        Err(e) => e.message,
        Ok(v) => panic!("{expr}: expected an error, got {v:?}"),
    }
}

/// Midnight UTC of a valid calendar date, as Unix seconds.
fn ymd(y: i32, m: u32, d: u32) -> i64 {
    chrono::NaiveDate::from_ymd_opt(y, m, d)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp()
}

// ---- LEN -----------------------------------------------------------------------------------

#[test]
fn len_counts_characters() {
    // Microsoft's LEN examples.
    assert_eq!(num("=LEN('Phoenix, AZ')"), 11.0);
    assert_eq!(num("=LEN('')"), 0.0);
    assert_eq!(num("=LEN('     One   ')"), 11.0);
    // Characters, not bytes.
    assert_eq!(num("=LEN('héllo')"), 5.0);
    // Excel converts numbers and booleans to text; an empty cell has length 0.
    assert_eq!(num("=LEN(12345)"), 5.0);
    assert_eq!(num("=LEN(TRUE)"), 4.0);
    assert_eq!(num("=LEN(NULL)"), 0.0);
}

#[test]
fn len_errors() {
    assert_eq!(err("=LEN([1, 2])"), "LEN expects text");
    assert_eq!(err("=LEN()"), "LEN expects 1 argument: text");
    assert_eq!(err("=LEN('a', 'b')"), "LEN expects 1 argument: text");
}

// ---- TODAY ---------------------------------------------------------------------------------

#[test]
fn today_is_date_with_no_arguments() {
    let today = date("=TODAY()");
    let date_now = date("=DATE()");
    // Same clock and zone as DATE(); allow for the two calls straddling midnight.
    assert!(today == date_now || today + 86_400 == date_now);
    assert_eq!(today % 86_400, 0, "midnight");
    assert_eq!(err("=TODAY(1)"), "TODAY expects no arguments");
}

// ---- PPMT ----------------------------------------------------------------------------------

#[test]
fn ppmt_matches_excel() {
    // Microsoft's PPMT examples.
    close("=PPMT(0.1 / 12, 1, 2 * 12, 2000)", -75.62);
    close("=PPMT(0.08, 10, 10, 200000)", -27598.05);
    // PMT(0.1, 3, 1000) = -402.11; interest in period 1 is -100, so principal is -302.11.
    close("=PPMT(0.1, 1, 3, 1000)", -302.11);
    // Period 2: balance 697.89 -> interest -69.79 -> principal -332.33.
    close("=PPMT(0.1, 2, 3, 1000)", -332.33);
    close("=PPMT(0.1, 3, 3, 1000)", -365.56);
    // Payments in advance (type 1): PMT = -365.56; no interest in period 1.
    close("=PPMT(0.1, 1, 3, 1000, 0, 1)", -365.56);
    close("=PPMT(0.1, 2, 3, 1000, 0, 1)", -302.11);
    // Zero rate: the payment is all principal.
    close("=PPMT(0, 1, 4, 1000)", -250.0);
    // With a future value: PMT(0.05, 10, 0, 1000) = -79.50, all principal in period 1.
    close("=PPMT(0.05, 1, 10, 0, 1000)", -79.50);
}

#[test]
fn ppmt_principal_sums_to_the_loan() {
    for t in [0, 1] {
        let total: f64 = (1..=12)
            .map(|per| num(&format!("=PPMT(0.01, {per}, 12, 5000, 0, {t})")))
            .sum();
        assert!((total + 5000.0).abs() < 1e-6, "type {t}: {total}");
    }
}

#[test]
fn ppmt_errors() {
    // per outside 1..=nper is Excel #NUM!.
    assert_eq!(
        err("=PPMT(0.1, 0, 3, 1000)"),
        "PPMT per must be between 1 and nper"
    );
    assert_eq!(
        err("=PPMT(0.1, 4, 3, 1000)"),
        "PPMT per must be between 1 and nper"
    );
    assert_eq!(
        err("=PPMT('x', 1, 3, 1000)"),
        "PPMT rate: cannot convert \"x\" to a number (#VALUE!)"
    );
    assert_eq!(
        err("=PPMT(0.1, 1, 3, 1000, 'x')"),
        "PPMT fv: cannot convert \"x\" to a number (#VALUE!)"
    );
    assert_eq!(
        err("=PPMT(0.1, 1, 3, [1000])"),
        "PPMT pv must be a number, got array"
    );
    assert!(err("=PPMT(0.1, 1, 3)").starts_with("PPMT expects 4-6 arguments"));
}

// ---- PV ------------------------------------------------------------------------------------

#[test]
fn pv_matches_excel() {
    // Microsoft's PV example: an annuity paying 500 a month for 20 years at 8%.
    close("=PV(0.08 / 12, 12 * 20, 500)", -59777.15);
    // 100 a year for 10 years at 5%: 100 * (1 - 1.05^-10) / 0.05 = 772.17.
    close("=PV(0.05, 10, -100)", 772.17);
    // Payments in advance: 772.17 * 1.05 = 810.78.
    close("=PV(0.05, 10, -100, 0, 1)", 810.78);
    // A lump sum: 1000 / 1.05^10 = 613.91.
    close("=PV(0.05, 10, 0, -1000)", 613.91);
    // Zero rate: -(fv + pmt * nper).
    close("=PV(0, 10, -100)", 1000.0);
    close("=PV(0, 10, -100, -500)", 1500.0);
}

#[test]
fn pv_errors() {
    assert_eq!(
        err("=PV(0.05, 10, 'x')"),
        "PV pmt: cannot convert \"x\" to a number (#VALUE!)"
    );
    assert_eq!(
        err("=PV(0.05, 10, -100, 'x')"),
        "PV fv: cannot convert \"x\" to a number (#VALUE!)"
    );
    assert!(err("=PV(0.05, 10)").starts_with("PV expects 3-5 arguments"));
    // rate = -1 divides by zero (Excel #NUM!/#DIV/0!).
    assert_eq!(err("=PV(-1, 10, -100)"), "PV result is not a finite number");
}

// ---- NPV -----------------------------------------------------------------------------------

#[test]
fn npv_matches_excel() {
    // Microsoft's NPV examples.
    close("=NPV(0.1, -10000, 3000, 4200, 6800)", 1188.44);
    close(
        "=NPV(0.08, 8000, 9200, 10000, 12000, 14500) + -40000",
        1922.06,
    );
    close(
        "=NPV(0.08, 8000, 9200, 10000, 12000, 14500, -9000) + -40000",
        -3749.47,
    );
    // Arrays are flattened in order.
    close("=NPV(0.1, [-10000, 3000], [4200, 6800])", 1188.44);
    // Only numbers in arrays count.
    close(
        "=NPV(0.1, [-10000, 'x', 3000, NULL, 4200, TRUE, 6800])",
        1188.44,
    );
    // Zero rate is the plain sum.
    close("=NPV(0, 1, 2, 3)", 6.0);
}

#[test]
fn npv_errors() {
    assert_eq!(err("=NPV(0.1)"), "NPV expects rate, value1, [value2], ...");
    assert_eq!(err("=NPV(0.1, [])"), "NPV expects at least one number");
    assert_eq!(
        err("=NPV('x', 1)"),
        "NPV rate: cannot convert \"x\" to a number (#VALUE!)"
    );
    assert_eq!(err("=NPV(-1, 100)"), "NPV result is not a finite number");
}

// ---- TEXTJOIN ------------------------------------------------------------------------------

#[test]
fn textjoin_matches_excel() {
    // Microsoft's first TEXTJOIN example.
    assert_eq!(
        text("=TEXTJOIN(', ', TRUE, 'US Dollar', 'Australian Dollar', 'Chinese Yuan', 'Hong Kong Dollar', 'Israeli Shekel', 'South Korean Won', 'Russian Ruble')"),
        "US Dollar, Australian Dollar, Chinese Yuan, Hong Kong Dollar, Israeli Shekel, South Korean Won, Russian Ruble"
    );
    // ignore_empty TRUE skips empty strings and NULLs; FALSE keeps them.
    assert_eq!(text("=TEXTJOIN(',', TRUE, 'a', '', NULL, 'b')"), "a,b");
    assert_eq!(text("=TEXTJOIN(',', FALSE, 'a', '', NULL, 'b')"), "a,,,b");
    // Arrays are flattened; numbers and booleans become text.
    assert_eq!(
        text("=TEXTJOIN('-', TRUE, [1, [2, 3]], TRUE, 2.5)"),
        "1-2-3-TRUE-2.5"
    );
    // An array delimiter is used in turn (Microsoft's example with {",",";"}-style delimiters).
    assert_eq!(
        text("=TEXTJOIN([',', ';'], TRUE, 'a', 'b', 'c', 'd', 'e')"),
        "a,b;c,d;e"
    );
    // An empty delimiter concatenates; one item has no delimiter.
    assert_eq!(text("=TEXTJOIN('', TRUE, 'a', 'b')"), "ab");
    assert_eq!(text("=TEXTJOIN(', ', TRUE, 'only')"), "only");
    // Numbers count as TRUE/FALSE for ignore_empty, as in Excel.
    assert_eq!(text("=TEXTJOIN(',', 0, 'a', '', 'b')"), "a,,b");
}

#[test]
fn textjoin_errors() {
    let usage = "TEXTJOIN expects delimiter, ignore_empty, text1, [text2], ...";
    assert_eq!(err("=TEXTJOIN(',', TRUE)"), usage);
    assert_eq!(err("=TEXTJOIN(',')"), usage);
    assert_eq!(
        err("=TEXTJOIN(',', 'yes', 'a')"),
        "TEXTJOIN ignore_empty must be TRUE or FALSE"
    );
    // A date is text since ADR-0031; a JSON object still has no text form.
    assert_eq!(
        err("=TEXTJOIN(',', TRUE, {\"a\": 1})"),
        "TEXTJOIN expects text, numbers, booleans, dates or arrays"
    );
}

// ---- PERCENTILE, QUARTILE, MODE (legacy Excel names) ---------------------------------------

#[test]
fn legacy_statistical_names_match_their_inc_and_sngl_forms() {
    // Microsoft's examples.
    close("=PERCENTILE([1, 3, 2, 4], 0.3)", 1.9);
    close("=QUARTILE([1, 2, 4, 7, 8, 9, 10, 12], 1)", 3.5);
    assert_eq!(num("=MODE(5.6, 4, 4, 3, 2, 4)"), 4.0);
    // Same results as the names they alias.
    for (a, b) in [
        (
            "PERCENTILE([1, 2, 3, 4], 0.4)",
            "PERCENTILE_INC([1, 2, 3, 4], 0.4)",
        ),
        (
            "QUARTILE([1, 2, 3, 4, 5], 3)",
            "QUARTILE_INC([1, 2, 3, 4, 5], 3)",
        ),
        ("MODE([3, 1, 1, 3])", "MODE_SNGL([3, 1, 1, 3])"),
    ] {
        assert_eq!(
            evaluate(&format!("={a}")),
            evaluate(&format!("={b}")),
            "{a}"
        );
    }
    assert_eq!(
        err("=PERCENTILE([1, 2], 1.5)"),
        "Percentile must be between 0 and 1 (#NUM!)"
    );
    assert_eq!(err("=QUARTILE([1, 2], 5)"), "Quartile must be 0-4 (#NUM!)");
}

// ---- FIND on text (D-6) --------------------------------------------------------------------

#[test]
fn find_on_text_is_excel_find() {
    // Microsoft's FIND examples.
    assert_eq!(num("=FIND('M', 'Miriam McGovern')"), 1.0);
    assert_eq!(num("=FIND('m', 'Miriam McGovern')"), 6.0);
    assert_eq!(num("=FIND('M', 'Miriam McGovern', 3)"), 8.0);
    // Substrings, characters (not bytes), start_num truncated.
    assert_eq!(num("=FIND('ana', 'banana')"), 2.0);
    assert_eq!(num("=FIND('ana', 'banana', 3)"), 4.0);
    assert_eq!(num("=FIND('a', 'banana', 3.9)"), 4.0);
    assert_eq!(num("=FIND('é', 'hé hé', 3)"), 5.0);
    // An empty find_text matches at start_num.
    assert_eq!(num("=FIND('', 'abc')"), 1.0);
    assert_eq!(num("=FIND('', 'abc', 3)"), 3.0);
    // within_text may be a number (converted to text, as in Excel).
    assert_eq!(num("=FIND('3', 12345)"), 3.0);
}

#[test]
fn find_on_text_errors() {
    let not_found = "FIND: find_text not found in within_text";
    // Case-sensitive: no match is Excel #VALUE!.
    assert_eq!(err("=FIND('B', 'abc')"), not_found);
    assert_eq!(
        err("=FIND('a', 'banana', 7)"),
        err("=FIND('a', 'banana', 0)")
    );
    assert_eq!(err("=FIND('n', 'banana', 6)"), not_found);
    let range = "FIND start_num must be between 1 and the length of within_text";
    assert_eq!(err("=FIND('a', 'abc', 0)"), range);
    assert_eq!(err("=FIND('a', 'abc', 4)"), range);
    assert_eq!(err("=FIND('a', 'abc', -1)"), range);
    assert_eq!(
        err("=FIND('a', 'abc', 'x')"),
        "FIND start_num must be a number"
    );
    assert_eq!(
        err("=FIND('a', [1, 2])"),
        "FIND expects within_text as text"
    );
}

#[test]
fn find_on_arrays_is_unchanged() {
    assert_eq!(num("=FIND([1, 5, 10], :x > 3)"), 5.0);
    assert_eq!(evaluate("=FIND([1, 2], :x > 5)").unwrap(), Value::Null);
    assert_eq!(err("=FIND(5, :x > 3)"), "FIND first arg must be array");
}

// ---- DATE rollover (D-5) -------------------------------------------------------------------

#[test]
fn date_rolls_months_and_days_over_like_excel() {
    // The brief's cases.
    assert_eq!(date("=DATE(2026, 13, 1)"), ymd(2027, 1, 1));
    assert_eq!(date("=DATE(2026, 2, 30)"), ymd(2026, 3, 2));
    assert_eq!(date("=DATE(2026, 1, 0)"), ymd(2025, 12, 31));
    assert_eq!(date("=DATE(2026, 0, 1)"), ymd(2025, 12, 1));
    // Microsoft's DATE examples.
    assert_eq!(date("=DATE(2008, 14, 2)"), ymd(2009, 2, 2));
    assert_eq!(date("=DATE(2008, -3, 2)"), ymd(2007, 9, 2));
    assert_eq!(date("=DATE(2008, 1, 35)"), ymd(2008, 2, 4));
    assert_eq!(date("=DATE(2008, 1, -15)"), ymd(2007, 12, 16));
    // End of month with DATE(y, m + 1, 0), leap years included.
    assert_eq!(date("=DATE(2024, 3, 0)"), ymd(2024, 2, 29));
    assert_eq!(date("=DATE(2026, 12 + 1, 0)"), ymd(2026, 12, 31));
    // Several years of months and days at once.
    assert_eq!(date("=DATE(2026, -13, 1)"), ymd(2024, 11, 1));
    assert_eq!(date("=DATE(2026, 1, 366)"), ymd(2027, 1, 1));
}

#[test]
fn date_years_below_1900_get_1900_added() {
    // Microsoft: DATE(108, 1, 2) is January 2, 2008.
    assert_eq!(date("=DATE(108, 1, 2)"), ymd(2008, 1, 2));
    assert_eq!(date("=DATE(26, 1, 1)"), ymd(1926, 1, 1));
    assert_eq!(date("=DATE(0, 1, 1)"), ymd(1900, 1, 1));
    assert_eq!(date("=DATE(1899, 12, 31)"), ymd(3799, 12, 31));
    assert_eq!(date("=DATE(1900, 1, 1)"), ymd(1900, 1, 1));
}

#[test]
fn date_truncates_arguments() {
    assert_eq!(date("=DATE(2026.9, 1.9, 1.9)"), ymd(2026, 1, 1));
    assert_eq!(date("=DATE(2026, 2.5, 28.99)"), ymd(2026, 2, 28));
}

#[test]
fn date_errors() {
    // Excel #NUM!: year below 0 or above 9999, or a result outside Excel's date range.
    let year = "DATE year must be between 0 and 9999";
    assert_eq!(err("=DATE(-1, 1, 1)"), year);
    assert_eq!(err("=DATE(10000, 1, 1)"), year);
    let range = "DATE result is outside 1900-01-01..9999-12-31";
    assert_eq!(err("=DATE(9999, 12, 32)"), range);
    assert_eq!(err("=DATE(1900, 1, 0)"), range);
    assert_eq!(err("=DATE(2026, 1, 10 ^ 300)"), range);
    assert_eq!(err("=DATE(2026, -10 ^ 300, 1)"), range);
    // BL-066: numeric text converts (`DATE('2026', 1, 1)` is tested in number_coercion.rs).
    assert_eq!(
        err("=DATE('x', 1, 1)"),
        "DATE year: cannot convert \"x\" to a number (#VALUE!)"
    );
    assert_eq!(err("=DATE([2026], 1, 1)"), "DATE expects year as number");
    assert_eq!(err("=DATE(2026, 1, NULL)"), "DATE expects day as number");
    assert_eq!(err_nan("=DATE(:nan, 1, 1)"), "DATE expects year as number");
    // String casts stay strict: no rollover there.
    assert!(evaluate("='2026-02-30'::DateTime").is_err());
}

// ---- Date arithmetic in days (D-5) ---------------------------------------------------------

#[test]
fn date_plus_and_minus_days() {
    assert_eq!(date("=DATE(2026, 1, 31) + 1"), ymd(2026, 2, 1));
    assert_eq!(date("=1 + DATE(2026, 1, 31)"), ymd(2026, 2, 1));
    assert_eq!(date("=DATE(2026, 3, 1) - 1"), ymd(2026, 2, 28));
    assert_eq!(date("=DATE(2026, 1, 1) + 365"), ymd(2027, 1, 1));
    assert_eq!(date("=DATE(2026, 1, 1) + -1"), ymd(2025, 12, 31));
    // Fractional days are times of day: 0.5 day = 12:00, 0.25 day = 06:00.
    assert_eq!(date("=DATE(2026, 1, 1) + 0.5"), ymd(2026, 1, 1) + 43_200);
    assert_eq!(date("=DATE(2026, 1, 1) - 0.25"), ymd(2025, 12, 31) + 64_800);
    // Rounded to the nearest second (DateTime is whole seconds).
    assert_eq!(
        date("=DATE(2026, 1, 1) + 1 / 86400 * 0.6"),
        ymd(2026, 1, 1) + 1
    );
}

/// TIME is a fraction of a day (ADR-0018), so it adds to a date like Excel's `DATE + TIME`.
#[test]
fn date_plus_time_is_time_of_day() {
    assert_eq!(
        date("=DATE(2026, 1, 1) + TIME(12, 0, 0)"),
        ymd(2026, 1, 1) + 43_200
    );
}

#[test]
fn date_minus_date_is_days() {
    assert_eq!(num("=DATE(2026, 1, 2) - DATE(2026, 1, 1)"), 1.0);
    assert_eq!(num("=DATE(2026, 3, 1) - DATE(2026, 2, 1)"), 28.0);
    assert_eq!(num("=DATE(2025, 1, 1) - DATE(2026, 1, 1)"), -365.0);
    assert_eq!(num("=(DATE(2026, 1, 1) + 0.75) - DATE(2026, 1, 1)"), 0.75);
    // DATEDIFF agrees for whole days.
    assert_eq!(
        num("=DATE(2026, 3, 1) - DATE(2026, 1, 1)"),
        num("=DATEDIFF(DATE(2026, 1, 1), DATE(2026, 3, 1), 'days')")
    );
}

#[test]
fn other_date_arithmetic_is_still_an_error() {
    let e = "Arithmetic op on non-number";
    assert_eq!(err("=DATE(2026, 1, 1) + DATE(2026, 1, 1)"), e);
    assert_eq!(err("=1 - DATE(2026, 1, 1)"), e);
    assert_eq!(err("=DATE(2026, 1, 1) * 2"), e);
    assert_eq!(err("=DATE(2026, 1, 1) / 2"), e);
    assert_eq!(err("=DATE(2026, 1, 1) + [1]"), e);
    // ADR-0027: non-numeric text is #VALUE!; NULL and numeric text convert.
    assert_eq!(
        err("=DATE(2026, 1, 1) + 'x'"),
        "Cannot convert \"x\" to a number (#VALUE!)"
    );
    assert_eq!(
        err("=DATE(2026, 1, 1) + 10 ^ 300"),
        "Date arithmetic out of range"
    );
    assert_eq!(
        err_nan("=DATE(2026, 1, 1) + :nan"),
        "Date arithmetic out of range"
    );
}

#[test]
fn date_plus_time_adds_time_as_days() {
    // Holds whatever unit TIME returns: it is added as days and read back as days.
    assert_eq!(date("=DATE(2026, 1, 1) + TIME(0, 0, 0)"), ymd(2026, 1, 1));
    assert_eq!(
        num("=(DATE(2026, 1, 1) + TIME(12, 0, 0)) - DATE(2026, 1, 1)"),
        num("=TIME(12, 0, 0)")
    );
}
