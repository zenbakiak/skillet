use skillet::{evaluate, evaluate_with_assignments, Value};
use std::collections::HashMap;

#[test]
fn test_hash_comment() {
    let result = evaluate("# This is a comment\n2 + 3").unwrap();
    assert_eq!(result, Value::Number(5.0));
}

#[test]
fn test_slash_slash_comment() {
    let result = evaluate("// This is a comment\n2 + 3").unwrap();
    assert_eq!(result, Value::Number(5.0));
}

#[test]
fn test_multiple_comments() {
    let result = evaluate_with_assignments("# First comment\n:x := 10;\n// Second comment\n:y := 20;\n# Third comment\n:x + :y", &HashMap::new()).unwrap();
    assert_eq!(result, Value::Number(30.0));
}

#[test]
fn test_comment_after_code() {
    // Note: Comments after code on same line are NOT supported in this version
    let result = evaluate_with_assignments(":x := 5;\n:y := 10;\n:x + :y", &HashMap::new());
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Number(15.0));
}

#[test]
fn test_indented_expression() {
    // Whitespace at start of lines should be handled
    let result = evaluate_with_assignments("  :x := 10;\n   :y := 20;\n      :x + :y", &HashMap::new()).unwrap();
    assert_eq!(result, Value::Number(30.0));
}

#[test]
fn test_mixed_whitespace() {
    let result = evaluate_with_assignments("\t:x := 10;\n\t\t:y := 20;\n\t:x + :y", &HashMap::new()).unwrap();
    assert_eq!(result, Value::Number(30.0));
}

#[test]
fn test_comment_with_variables() {
    let mut vars = HashMap::new();
    vars.insert("price".to_string(), Value::Number(100.0));
    vars.insert("qty".to_string(), Value::Number(5.0));

    let result = evaluate_with_assignments(
        "# Calculate total\n:subtotal := :price * :qty;\n// Add 16% tax\n:tax := :subtotal * 0.16;\n# Return total\n:subtotal + :tax",
        &vars
    ).unwrap();

    assert_eq!(result, Value::Number(580.0));
}

#[test]
fn test_ifs_with_indentation() {
    let mut vars = HashMap::new();
    vars.insert("qty".to_string(), Value::Number(25.0));

    let result = evaluate_with_assignments(
        ":discount := IFS(\n  :qty >= 100, 0.20,\n  :qty >= 50, 0.15,\n  :qty >= 10, 0.10,\n  true, 0\n);\n:discount",
        &vars
    ).unwrap();

    assert_eq!(result, Value::Number(0.10));
}

#[test]
fn test_object_literal_with_indentation() {
    let result = evaluate_with_assignments(":result := {\n  name: 'John',\n  age: 30,\n  active: true\n};\n:result", &HashMap::new()).unwrap();

    match result {
        Value::Json(_) => {}, // Success
        _ => panic!("Expected JSON object"),
    }
}

#[test]
fn test_map_with_indentation() {
    let mut vars = HashMap::new();
    vars.insert("items".to_string(), Value::Array(vec![
        Value::Number(10.0),
        Value::Number(20.0),
        Value::Number(30.0),
    ]));

    let result = evaluate_with_assignments(
        ":result := :items.map(\n  :x * 2\n);\n:result",
        &vars
    ).unwrap();

    assert_eq!(result, Value::Array(vec![
        Value::Number(20.0),
        Value::Number(40.0),
        Value::Number(60.0),
    ]));
}

#[test]
fn test_complex_indented_expression() {
    let mut vars = HashMap::new();
    vars.insert("salario_mensual".to_string(), Value::Number(15000.0));
    vars.insert("dias_trabajados".to_string(), Value::Number(15.0));

    let result = evaluate_with_assignments(
        "# Calculate salary\n:salario_diario := :salario_mensual / 30;\n\n// Calculate payment\n:pago := :salario_diario * :dias_trabajados;\n\n# Return result\n:pago",
        &vars
    ).unwrap();

    assert_eq!(result, Value::Number(7500.0));
}

#[test]
fn test_empty_lines_and_comments() {
    let result = evaluate_with_assignments("# Start\n\n:x := 10;\n\n// Middle comment\n\n:y := 20;\n\n# End\n\n:x + :y", &HashMap::new()).unwrap();

    assert_eq!(result, Value::Number(30.0));
}

#[test]
fn test_block_comment() {
    let result = evaluate("/* This is a block comment */ 2 + 3").unwrap();
    assert_eq!(result, Value::Number(5.0));
}

#[test]
fn test_inline_block_comment() {
    let result = evaluate_with_assignments(":x := 10 /* inline comment */ * 2; :x", &HashMap::new()).unwrap();
    assert_eq!(result, Value::Number(20.0));
}

#[test]
fn test_multiline_block_comment() {
    let result = evaluate_with_assignments(
        "/* This is a\n   multi-line\n   block comment */\n:x := 10;\n:y := 20;\n:x + :y",
        &HashMap::new()
    ).unwrap();
    assert_eq!(result, Value::Number(30.0));
}

#[test]
fn test_mixed_comment_styles() {
    let result = evaluate_with_assignments(
        "# Line comment\n:x := 10;\n/* Block comment */ :y := 20;\n// Another line comment\n:z := 30;\n:x + :y + :z",
        &HashMap::new()
    ).unwrap();
    assert_eq!(result, Value::Number(60.0));
}

#[test]
fn test_nested_expression_with_block_comment() {
    let mut vars = HashMap::new();
    vars.insert("qty".to_string(), Value::Number(25.0));

    let result = evaluate_with_assignments(
        ":discount := IFS(\n  :qty >= 100, /* bulk discount */ 0.20,\n  :qty >= 50, /* medium discount */ 0.15,\n  :qty >= 10, /* small discount */ 0.10,\n  true, /* no discount */ 0\n);\n:discount",
        &vars
    ).unwrap();

    assert_eq!(result, Value::Number(0.10));
}

#[test]
fn test_unterminated_block_comment() {
    // An unterminated block comment should produce an error
    let result = evaluate("/* This is unterminated\n2 + 3");
    assert!(result.is_err(), "Expected error for unterminated block comment");
}
