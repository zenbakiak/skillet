use skillet::{evaluate, Value};

fn as_number(v: Value) -> f64 {
    match v {
        Value::Number(n) => n,
        _ => panic!("Expected number, got {:?}", v),
    }
}

fn as_datetime(v: Value) -> i64 {
    match v {
        Value::DateTime(ts) => ts,
        _ => panic!("Expected datetime, got {:?}", v),
    }
}

fn as_bool(v: Value) -> bool {
    match v {
        Value::Boolean(b) => b,
        _ => panic!("Expected boolean, got {:?}", v),
    }
}

fn as_string(v: Value) -> String {
    match v {
        Value::String(s) => s,
        _ => panic!("Expected string, got {:?}", v),
    }
}

#[test]
fn test_now_function() {
    let result = evaluate("=NOW()").unwrap();
    // Should return a DateTime timestamp
    assert!(matches!(result, Value::DateTime(_)));

    // Timestamp should be reasonable (after 2020 and before 2030)
    let timestamp = as_datetime(result);
    assert!(timestamp > 1577836800); // 2020-01-01
    assert!(timestamp < 1893456000); // 2030-01-01
}

#[test]
fn test_date_function() {
    let result = evaluate("=DATE()").unwrap();
    // Should return a DateTime timestamp representing today at midnight
    assert!(matches!(result, Value::DateTime(_)));
}

#[test]
fn test_time_function() {
    let result = evaluate("=TIME()").unwrap();
    // Should return seconds since midnight
    let seconds = as_number(result);
    assert!(seconds >= 0.0);
    assert!(seconds < 86400.0); // Less than 24 hours in seconds
}

#[test]
fn test_year_month_day_functions() {
    // Test with NOW()
    let year = as_number(evaluate("=YEAR(NOW())").unwrap());
    let month = as_number(evaluate("=MONTH(NOW())").unwrap());
    let day = as_number(evaluate("=DAY(NOW())").unwrap());

    // Should be current year (around 2025)
    assert!((2024.0..=2026.0).contains(&year));

    // Month should be 1-12
    assert!((1.0..=12.0).contains(&month));

    // Day should be 1-31
    assert!((1.0..=31.0).contains(&day));
}

#[test]
fn test_dateadd_function() {
    // Test adding days
    let now = as_datetime(evaluate("=NOW()").unwrap());
    let future = as_datetime(evaluate("=DATEADD(NOW(), 7, \"days\")").unwrap());
    let diff_days = (future - now) / 86400; // Convert to days
    assert!((diff_days - 7).abs() < 1); // Should be approximately 7 days

    // Test adding hours
    let future_hours = as_datetime(evaluate("=DATEADD(NOW(), 24, \"hours\")").unwrap());
    let diff_hours = (future_hours - now) / 3600; // Convert to hours
    assert!((diff_hours - 24).abs() < 1); // Should be approximately 24 hours
}

#[test]
fn test_datediff_function() {
    // Test difference in days
    let diff =
        as_number(evaluate("=DATEDIFF(NOW(), DATEADD(NOW(), 7, \"days\"), \"days\")").unwrap());
    assert_eq!(diff, 7.0);

    // Test difference in hours
    let diff_hours =
        as_number(evaluate("=DATEDIFF(NOW(), DATEADD(NOW(), 24, \"hours\"), \"hours\")").unwrap());
    assert_eq!(diff_hours, 24.0);

    // Test reverse (should be negative)
    let diff_reverse =
        as_number(evaluate("=DATEDIFF(DATEADD(NOW(), 7, \"days\"), NOW(), \"days\")").unwrap());
    assert_eq!(diff_reverse, -7.0);
}

#[test]
fn test_substring_function() {
    // Basic substring
    assert_eq!(
        as_string(evaluate("=SUBSTRING(\"Hello World\", 0, 5)").unwrap()),
        "Hello"
    );

    // Substring without length (to end)
    assert_eq!(
        as_string(evaluate("=SUBSTRING(\"Hello World\", 6)").unwrap()),
        "World"
    );

    // Unicode support
    assert_eq!(
        as_string(evaluate("=SUBSTRING(\"Hello 🌍\", 6, 2)").unwrap()),
        "🌍"
    );

    // Out of bounds (should return empty string)
    assert_eq!(
        as_string(evaluate("=SUBSTRING(\"Hello\", 10, 5)").unwrap()),
        ""
    );

    // Zero length
    assert_eq!(
        as_string(evaluate("=SUBSTRING(\"Hello\", 0, 0)").unwrap()),
        ""
    );
}

#[test]
fn test_type_checking_functions() {
    // ISNUMBER tests
    assert!(as_bool(evaluate("=ISNUMBER(42)").unwrap()));
    assert!(as_bool(evaluate("=ISNUMBER(3.14)").unwrap()));
    assert!(!as_bool(evaluate("=ISNUMBER(\"hello\")").unwrap()));
    assert!(!as_bool(evaluate("=ISNUMBER(TRUE)").unwrap()));
    assert!(!as_bool(evaluate("=ISNUMBER(NULL)").unwrap()));

    // ISTEXT tests
    assert!(as_bool(evaluate("=ISTEXT(\"hello\")").unwrap()));
    assert!(as_bool(evaluate("=ISTEXT(\"\")").unwrap()));
    assert!(!as_bool(evaluate("=ISTEXT(42)").unwrap()));
    assert!(!as_bool(evaluate("=ISTEXT(TRUE)").unwrap()));
    assert!(!as_bool(evaluate("=ISTEXT(NULL)").unwrap()));
}

#[test]
fn test_complex_datetime_expressions() {
    // Calculate age in years (using a fixed date for deterministic testing)
    // This is a conceptual test - in practice you'd use actual birth dates
    let years_diff =
        as_number(evaluate("=DATEDIFF(DATEADD(NOW(), -365, \"days\"), NOW(), \"years\")").unwrap());
    assert!((0.0..=2.0).contains(&years_diff)); // Should be about 1 year

    // Test chaining date functions
    let next_year = as_number(evaluate("=YEAR(DATEADD(NOW(), 1, \"years\"))").unwrap());
    let current_year = as_number(evaluate("=YEAR(NOW())").unwrap());
    assert_eq!(next_year, current_year + 1.0);
}

#[test]
fn test_string_and_datetime_together() {
    // Test that we can extract parts of dates and use them in strings
    let year_str = as_string(evaluate("=CONCAT(\"Year: \", YEAR(NOW()))").unwrap());
    assert!(year_str.starts_with("Year: "));
    assert!(year_str.contains("202")); // Should contain 2024, 2025, etc.
}

#[test]
fn test_time_with_arguments_is_seconds_since_midnight() {
    // Regression (Excel audit B-1): TIME(h, m, s) used to ignore its arguments and return the
    // current local time. docs/reference/api.md: TIME(14, 30, 0) = 52200.
    // 14*3600 + 30*60 + 0 = 50400 + 1800 = 52200
    assert_eq!(as_number(evaluate("=TIME(14, 30, 0)").unwrap()), 52200.0);
    // 0*3600 + 0*60 + 0 = 0
    assert_eq!(as_number(evaluate("=TIME(0, 0, 0)").unwrap()), 0.0);
    // 23*3600 + 59*60 + 59 = 82800 + 3540 + 59 = 86399
    assert_eq!(as_number(evaluate("=TIME(23, 59, 59)").unwrap()), 86399.0);
    // Wrong arity or non-numeric parts are errors.
    assert!(evaluate("=TIME(1)").is_err());
    assert!(evaluate("=TIME(1, 2)").is_err());
    assert!(evaluate("=TIME('a', 0, 0)").is_err());
}
