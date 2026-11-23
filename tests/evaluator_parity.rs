//! Differential tests between two entry layers of the evaluator.
//!
//! - runtime layer: the expression is parsed once with `skillet::parse` and evaluated with
//!   `skillet::runtime::evaluator::*` directly.
//! - public layer: the source string goes through the top-level API in `src/lib.rs`
//!   (`evaluate`, `evaluate_with`, `evaluate_with_custom`, `evaluate_with_assignments`,
//!   `evaluate_with_assignments_and_context`), which parses and dispatches on its own.
//!
//! Both must agree for every expression (both `Ok` with the same `Value`, or both `Err`).
//! Error messages are not compared, but a differing message is reported on stderr so it shows
//! up with `--nocapture`.
//!
//! History: BL-001a wrote this file to compare `runtime::evaluator` with the duplicate
//! `evaluation/` evaluator. BL-001b merged them and ADR-0010 deleted the old module path,
//! so the second side is now the public API. The `divergence_*` tests at the bottom keep the
//! cases (D1-D5) where the old duplicate disagreed, as regression tests.
//!
//! Since BL-087 (QA-1) the public layer's outcome for every (expression, mode) pair is also
//! pinned in `tests/golden/evaluator_parity.txt` (see `golden_values_for_the_whole_corpus`).
//! The parity comparison stays: it is cheap, and it still guards the `src/lib.rs` wrappers
//! (parse, limits, the global vs an explicit registry in custom mode) against drifting from
//! the runtime entry points. The golden file is what catches a change in meaning.
//!
//! To extend the corpus, add a string to the relevant table, then run
//! `UPDATE_GOLDEN=1 cargo test --test evaluator_parity golden` and commit the new lines.

use skillet::runtime::evaluator;
use skillet::{parse, CustomFunction, Error, FunctionRegistry, Value};
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Once, RwLock};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn n(x: f64) -> Value {
    Value::Number(x)
}
fn st(x: &str) -> Value {
    Value::String(x.to_string())
}
fn arr(xs: &[f64]) -> Value {
    Value::Array(xs.iter().map(|x| n(*x)).collect())
}

#[allow(deprecated)] // `money` is a host Currency, read as a Number (ADR-0033 §5)
fn fixture_vars() -> HashMap<String, Value> {
    let mut v = HashMap::new();
    v.insert("a".into(), n(10.0));
    v.insert("b".into(), n(3.0));
    v.insert("neg".into(), n(-4.5));
    v.insert("s".into(), st("Hello World"));
    v.insert("blank".into(), st(""));
    v.insert("num_text".into(), st("42"));
    v.insert("arr".into(), arr(&[1.0, 2.0, 3.0, 4.0, 5.0]));
    v.insert("empty".into(), Value::Array(vec![]));
    v.insert(
        "words".into(),
        Value::Array(vec![st("b"), st("a"), st("c")]),
    );
    v.insert("nil".into(), Value::Null);
    v.insert("flag".into(), Value::Boolean(true));
    v.insert("money".into(), Value::Currency(19.99));
    // Fixed instant: 2024-01-15T00:00:00Z
    v.insert("when".into(), Value::DateTime(1_705_276_800));
    v.insert(
        "obj".into(),
        Value::Json(r#"{"user":{"name":"Ann","age":31,"tags":["x","y"]},"items":[{"p":10},{"p":20},{"p":5}]}"#.into()),
    );
    v.insert(
        "arguments".into(),
        Value::Json(r#"{"accounts":[{"amount":100},{"amount":250}],"name":"acme"}"#.into()),
    );
    v
}

struct Triple;
impl CustomFunction for Triple {
    fn name(&self) -> &str {
        "TRIPLE"
    }
    fn min_args(&self) -> usize {
        1
    }
    fn max_args(&self) -> Option<usize> {
        Some(1)
    }
    fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
        let x = args[0]
            .as_number()
            .ok_or_else(|| Error::new("TRIPLE expects a number", None))?;
        Ok(Value::Number(x * 3.0))
    }
}

struct Greet;
impl CustomFunction for Greet {
    fn name(&self) -> &str {
        "GREET"
    }
    fn min_args(&self) -> usize {
        0
    }
    fn max_args(&self) -> Option<usize> {
        Some(1)
    }
    fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
        match args.first() {
            Some(Value::String(s)) => Ok(Value::String(format!("hi {}", s))),
            Some(_) => Err(Error::new("GREET expects a string", None)),
            None => Ok(Value::String("hi".into())),
        }
    }
}

/// Registry passed explicitly to the runtime layer.
fn local_registry() -> Arc<RwLock<FunctionRegistry>> {
    let mut r = FunctionRegistry::new();
    r.register(Box::new(Triple)).unwrap();
    r.register(Box::new(Greet)).unwrap();
    Arc::new(RwLock::new(r))
}

/// The public layer (`skillet::evaluate_with_custom`) only reads the global registry, so the
/// same two functions are registered there once. Each integration test file is its own binary
/// and nothing else in this file touches the global registry, so this cannot race.
fn ensure_global_registry() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        skillet::register_function(Box::new(Triple)).unwrap();
        skillet::register_function(Box::new(Greet)).unwrap();
    });
}

// ---------------------------------------------------------------------------
// Differential harness
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
enum Mode {
    /// `eval(expr)` vs `evaluate(src)` — no variables.
    Eval,
    /// `eval_with_vars(expr, fixture)` vs `evaluate_with(src, fixture)`.
    Vars,
    /// `eval_with_vars_and_custom(expr, fixture, local registry)` vs
    /// `evaluate_with_custom(src, fixture)` (global registry with the same functions).
    Custom,
    /// `eval_with_assignments(expr, fixture)` vs `evaluate_with_assignments(src, fixture)`.
    Assign,
    /// `eval_with_assignments_and_context` vs `evaluate_with_assignments_and_context` —
    /// compares value and final vars.
    Context,
}

/// Outcome normalised to a comparable string. `Debug` equality (rather than `PartialEq`)
/// keeps NaN comparable and prints nicely in failure reports.
#[derive(Debug)]
enum Outcome {
    Ok(String),
    Err(String),
    Panic,
}

impl Outcome {
    fn agrees(&self, other: &Outcome) -> bool {
        match (self, other) {
            (Outcome::Ok(a), Outcome::Ok(b)) => a == b,
            (Outcome::Err(_), Outcome::Err(_)) => true,
            (Outcome::Panic, Outcome::Panic) => true,
            _ => false,
        }
    }
}

/// Final context rendered as only the keys that were added, changed or removed relative to the
/// fixture, in sorted order (keeps failure reports short).
fn sorted_vars(vars: &HashMap<String, Value>) -> String {
    let base = fixture_vars();
    let mut keys: Vec<&String> = vars.keys().chain(base.keys()).collect();
    keys.sort();
    keys.dedup();
    keys.iter()
        .filter(|k| vars.get(**k) != base.get(**k))
        .map(|k| format!("{}={:?}", k, vars.get(*k)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn run(f: impl FnOnce() -> Result<String, Error>) -> Outcome {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(v)) => Outcome::Ok(v),
        Ok(Err(e)) => Outcome::Err(e.message),
        Err(_) => Outcome::Panic,
    }
}

/// Evaluates `src` through both layers. Returns `None` when they agree, otherwise a
/// one-line description of the divergence.
fn diverges(src: &str, mode: Mode) -> Option<String> {
    let expr = match parse(src) {
        Ok(e) => e,
        Err(e) => panic!("corpus expression does not parse: {:?}: {}", src, e.message),
    };
    let (runtime, public) = layers(src, &expr, mode);
    if let (Outcome::Err(a), Outcome::Err(b)) = (&runtime, &public) {
        if a != b {
            eprintln!("[parity] {:?} {:?}: same outcome, different error messages: runtime={:?} public={:?}", mode, src, a, b);
        }
    }
    if runtime.agrees(&public) {
        None
    } else {
        Some(format!(
            "{:?} {:?}\n    runtime::evaluator : {:?}\n    public API         : {:?}",
            mode, src, runtime, public
        ))
    }
}

/// The outcome of `src` through the runtime layer and through the public layer.
fn layers(src: &str, expr: &skillet::Expr, mode: Mode) -> (Outcome, Outcome) {
    let vars = fixture_vars();
    let reg = local_registry();
    ensure_global_registry();
    match mode {
        Mode::Eval => (
            run(|| evaluator::eval(expr).map(|v| format!("{:?}", v))),
            run(|| skillet::evaluate(src).map(|v| format!("{:?}", v))),
        ),
        Mode::Vars => (
            run(|| evaluator::eval_with_vars(expr, &vars).map(|v| format!("{:?}", v))),
            run(|| skillet::evaluate_with(src, &vars).map(|v| format!("{:?}", v))),
        ),
        Mode::Custom => (
            run(|| {
                evaluator::eval_with_vars_and_custom(expr, &vars, &reg).map(|v| format!("{:?}", v))
            }),
            run(|| skillet::evaluate_with_custom(src, &vars).map(|v| format!("{:?}", v))),
        ),
        Mode::Assign => (
            run(|| evaluator::eval_with_assignments(expr, &vars).map(|v| format!("{:?}", v))),
            run(|| skillet::evaluate_with_assignments(src, &vars).map(|v| format!("{:?}", v))),
        ),
        Mode::Context => (
            run(|| {
                evaluator::eval_with_assignments_and_context(expr, &vars)
                    .map(|(v, ctx)| format!("{:?} | {}", v, sorted_vars(&ctx)))
            }),
            run(|| {
                skillet::evaluate_with_assignments_and_context(src, &vars)
                    .map(|(v, ctx)| format!("{:?} | {}", v, sorted_vars(&ctx)))
            }),
        ),
    }
}

fn is_known_divergence(src: &str, mode: Mode) -> bool {
    KNOWN_DIVERGENCES
        .iter()
        .flat_map(|t| t.iter())
        .any(|(s, m)| *s == src && std::mem::discriminant(m) == std::mem::discriminant(&mode))
}

/// Runs every expression in `table` through every mode in `modes` and fails with the full list
/// of divergences (not just the first). Pairs listed in `KNOWN_DIVERGENCES` are skipped here.
fn assert_parity(table: &[&str], modes: &[Mode]) {
    let failures: Vec<String> = table
        .iter()
        .flat_map(|src| {
            modes
                .iter()
                .filter(move |m| !is_known_divergence(src, **m))
                .filter_map(move |m| diverges(src, *m))
        })
        .collect();
    assert!(
        failures.is_empty(),
        "{} evaluator divergence(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// For divergence tables: asserts that the two layers agree on each listed (expression, mode).
fn assert_parity_each(table: &[(&str, Mode)]) {
    let failures: Vec<String> = table
        .iter()
        .filter_map(|(src, m)| diverges(src, *m))
        .collect();
    assert!(
        failures.is_empty(),
        "{} evaluator divergence(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

const ALL_VAR_MODES: &[Mode] = &[Mode::Vars, Mode::Custom, Mode::Assign, Mode::Context];
const ALL_MODES: &[Mode] = &[
    Mode::Eval,
    Mode::Vars,
    Mode::Custom,
    Mode::Assign,
    Mode::Context,
];

// ---------------------------------------------------------------------------
// Corpus: Expr variants (no variables needed)
// ---------------------------------------------------------------------------

const LITERALS_AND_OPERATORS: &[&str] = &[
    // Number / StringLit / Null
    "42",
    "-3.25",
    "'single'",
    "\"double\"",
    "NULL",
    "TRUE",
    "FALSE",
    // Unary
    "-5",
    "+5",
    "!TRUE",
    "-(2 + 3)",
    // Binary arithmetic + precedence
    "2 + 3 * 4",
    "(2 + 3) * 4",
    "10 - 4 - 3",
    "2 ^ 3 ^ 2",
    "-2 ^ 2",
    "7 % 3",
    "7 / 2",
    "1 / 0",
    "'a' + 'b'",
    "'n=' + 1",
    // Comparisons
    "2 > 1",
    "2 < 1",
    "2 >= 2",
    "2 <= 1",
    "2 == 2",
    "2 != 2",
    "'a' == 'a'",
    "NULL == NULL",
    // Logical
    "TRUE AND FALSE",
    "TRUE OR FALSE",
    "TRUE && TRUE",
    "FALSE || TRUE",
    "(2 > 1) AND (1 < 2)",
    "1 + 2 > 2 AND 3 < 4",
    // Array / Index / Slice / Spread
    "[1, 2, 3]",
    "[]",
    "[1, 'two', NULL, TRUE]",
    "[1, 2, 3][0]",
    "[1, 2, 3][-1]",
    "[1, 2, 3][10]",
    "[1, 2, 3, 4, 5][1:3]",
    "[1, 2, 3, 4, 5][:2]",
    "[1, 2, 3, 4, 5][2:]",
    "SUM(...[1, 2, 3])",
    // Spread in array literals (ADR-0005 amendment 1, BL-106)
    "[0, ...[1, 2, 3], 4]",
    "[...[]]",
    "[...[1, [2, 3]]]",
    "[...[1], ...['a', NULL]].length()",
    // ObjectLiteral / PropertyAccess / SafePropertyAccess
    "{\"a\": 1, \"b\": \"x\"}",
    "{}",
    "{\"a\": {\"b\": 2}}.a.b",
    "{\"a\": 1}&.a",
    "{\"a\": 1}&.missing",
    "NULL&.anything",
    // MethodCall / predicate / SafeMethodCall
    "'hello'.upper()",
    "(-3.7).abs().round(1)",
    "[1, 2, 3, 4].length().positive?",
    "0.zero?",
    "[].empty?",
    "NULL&.upper()",
    "'abc'&.upper()",
    // TypeCast
    "'42'::Integer",
    "TRUE::Float",
    "3.9::Integer",
    "123::String",
    "'true'::Boolean",
    "'abc'::Integer",
    // Sequence without assignment
    "1; 2; 3",
];

#[test]
fn parity_literals_and_operators() {
    assert_parity(LITERALS_AND_OPERATORS, ALL_MODES);
}

// ---------------------------------------------------------------------------
// Corpus: builtins, at least two per category in function_dispatch.rs
// ---------------------------------------------------------------------------

const BUILTINS: &[&str] = &[
    // arithmetic
    "SUM(1, 2, 3)",
    "SUM([1, 2], 3)",
    "AVG(2, 4, 6)",
    "AVERAGE([1, 2, 3, 4])",
    "MIN(3, 1, 2)",
    "MAX([3, 9, 2])",
    "ROUND(3.14159, 2)",
    "CEIL(2.1)",
    "FLOOR(-2.1)",
    "ABS(-7)",
    "SQRT(16)",
    "POW(2, 10)",
    "MOD(10, 3)",
    "INT(5.9)",
    "PRODUCT(2, 3, 4)",
    // logical
    "AND(TRUE, FALSE)",
    "OR(FALSE, TRUE)",
    "NOT(TRUE)",
    "XOR(TRUE, FALSE)",
    "IF(1 > 0, 'yes', 'no')",
    "IF(FALSE, 1)",
    "IFS(FALSE, 'a', TRUE, 'b')",
    // string
    "LENGTH('hello')",
    "CONCAT('a', 'b', 'c')",
    "UPPER('abc')",
    "LOWER('ABC')",
    "TRIM('  x  ')",
    "SUBSTRING('hello', 1, 3)",
    "SPLIT('a,b,c', ',')",
    "REPLACE('aaa', 'a', 'b')",
    "SUBSTITUTE('a-b-c', '-', '+')",
    "ISBLANK('')",
    "ISNUMBER(5)",
    "ISTEXT('x')",
    "INCLUDES('hello', 'ell')",
    "LEFT('hello', 2)",
    "RIGHT('hello', 2)",
    "MID('hello', 2, 3)",
    // array
    "ARRAY(1, 2, 3)",
    "FLATTEN([1, [2, [3]]])",
    "FIRST([9, 8, 7])",
    "LAST([9, 8, 7])",
    "CONTAINS([1, 2, 3], 2)",
    "IN([1, 2, 3], 4)",
    "COUNT([1, 2, 3])",
    "UNIQUE([1, 2, 2, 3])",
    "SORT([3, 1, 2])",
    "SORT([3, 1, 2], 'DESC')",
    "REVERSE([1, 2, 3])",
    "JOIN([1, 2, 3], '-')",
    "MERGE([1, 2], [3])",
    // datetime (fixed dates only)
    "DATE(2024, 1, 15)",
    "YEAR(DATE(2024, 3, 9))",
    "MONTH(DATE(2024, 3, 9))",
    "DAY(DATE(2024, 3, 9))",
    "DATEADD(DATE(2024, 1, 31), 1, 'days')",
    "DATEDIFF(DATE(2024, 1, 1), DATE(2024, 3, 1), 'days')",
    // financial
    "PMT(0.05 / 12, 360, 200000)",
    "FV(0.05, 10, -100)",
    "IPMT(0.1 / 12, 1, 36, 8000)",
    "DB(10000, 1000, 5, 1)",
    // statistical
    "MEDIAN(1, 3, 2, 4)",
    "MODE_SNGL(1, 2, 2, 3)",
    "STDEV_P(2, 4, 4, 4, 5, 5, 7, 9)",
    "VAR_P(1, 2, 3, 4)",
    "PERCENTILE_INC([1, 2, 3, 4], 0.5)",
    "QUARTILE_INC([1, 2, 3, 4, 5], 1)",
    // higher-order (literal arrays, default :x / :acc)
    "FILTER([1, 2, 3, 4], :x % 2 == 0)",
    "MAP([1, 2, 3], :x * 10)",
    "REDUCE([1, 2, 3], :acc + :x, 0)",
    "FIND([1, 5, 10], :x > 3)",
    "SUMIF([1, -2, 3], :x > 0)",
    "AVGIF([1, 3, 5, -1], :x > 0)",
    "COUNTIF([1, 2, 3, 4], :x % 2 == 0)",
    // higher-order with custom parameter names
    "FILTER([1, 2, 3, 4], :v > 2, 'v')",
    "MAP([1, 2, 3], :item + 1, 'item')",
    "REDUCE([1, 2, 3], :total + :x, 10, 'x', 'total')",
    "FIND([1, 5, 10], :n > 3, 'n')",
    // nested builtins
    "ROUND(AVG(SUM(1, 2), MAX(4, 5)), 1)",
    "UPPER(CONCAT('a', LOWER('B')))",
    // Aggregates: direct arguments convert, array items are skipped (ADR-0029 am. 2, BL-107)
    "SUM(1, \"2\", TRUE)",
    "SUM([1, \"2\", TRUE])",
    "SUM(...[1, \"2\", TRUE])",
    "SUM([...[1, \"2\", TRUE]])",
    "SUM(1, \"x\")",
    "SUM({\"a\": 1}, 2)",
    "AVERAGE(1, \"2\")",
    "MAX(-1, \"5\")",
    "COUNT(1, \"2\", TRUE, \"x\")",
    "MEDIAN(1, \"9\", \"8\")",
];

#[test]
fn parity_builtins() {
    assert_parity(BUILTINS, ALL_MODES);
}

// ---------------------------------------------------------------------------
// Corpus: variables, methods, lambdas over variables, JSON / JQ / DIG
// ---------------------------------------------------------------------------

const WITH_VARIABLES: &[&str] = &[
    // Variables
    ":a",
    ":a + :b * 2",
    ":s",
    ":nil",
    ":flag AND :a > 5",
    ":money",
    ":when",
    ":obj",
    // Property access on JSON variables
    ":obj.user.name",
    ":obj.user.tags",
    ":obj&.user&.name",
    ":obj&.user&.missing",
    ":obj&.nope&.deeper",
    ":nil&.name",
    ":obj.items.length()",
    // Index / slice on variables
    ":arr[0]",
    ":arr[-1]",
    ":arr[1:3]",
    ":words[1]",
    ":s[0]",
    // Spread of variable
    "SUM(...:arr)",
    "MAX(...:arr, 100)",
    "[0, ...:arr, 6]",
    "[...:empty, ...:words]",
    "[...:arr.map(:x * 2)].sum()",
    "[...:nil.to_a(), 1]",
    // Methods on variables
    ":s.upper()",
    ":s.lower().length()",
    ":arr.sum()",
    ":arr.avg()",
    ":arr.first()",
    ":arr.last()",
    ":arr.reverse()",
    ":arr.sort()",
    ":words.sort()",
    ":words.join('|')",
    ":arr.length()",
    ":empty.empty?",
    ":arr.empty?",
    ":neg.abs()",
    ":neg.negative?",
    ":a.positive?",
    ":nil.blank?",
    ":blank.blank?",
    ":num_text.to_i()",
    ":a.to_s()",
    ":arr.flatten()",
    ":arr.unique()",
    // Method lambdas, default :x / :acc
    ":arr.filter(:x > 2)",
    ":arr.map(:x * 2)",
    ":arr.reduce(:acc + :x, 0)",
    ":arr.find(:x > 3)",
    ":arr.filter(:x > 1).map(:x * 0.5).sum()",
    ":arr.filter(:x > 100)",
    ":empty.map(:x + 1)",
    // Method lambdas, custom param names
    ":arr.filter(:v > 2, 'v')",
    ":arr.map(:item * 3, 'item')",
    ":arr.reduce(:t + :x, 0, 'x', 't')",
    // Lambda referencing outer variables
    ":arr.filter(:x > :b)",
    ":arr.map(:x + :a)",
    "FILTER(:arr, :x > :b)",
    "MAP(:arr, :x * :a)",
    "REDUCE(:arr, :acc * :x, 1)",
    "SUMIF(:arr, :x > 2)",
    "COUNTIF(:arr, :x >= 3)",
    "AVGIF(:arr, :x < 3)",
    "FIND(:arr, :x == 4)",
    // Builtins over variables
    "SUM(:arr)",
    "AVG(:arr)",
    "COUNT(:arr)",
    "CONCAT(:s, '!')",
    "LENGTH(:s)",
    "ISBLANK(:nil)",
    "ISBLANK(:blank)",
    "IF(:flag, :a, :b)",
    "YEAR(:when)",
    "ROUND(:money, 0)",
    // JSON / JQ / DIG
    "JQ(:arguments, '$.accounts[*].amount')",
    "SUM(JQ(:arguments, '$.accounts[*].amount'))",
    "JQ(:arguments, '$.name')",
    "JQ(:obj, '$.items[*].p')",
    "DIG(:obj, ['user', 'name'])",
    "DIG(:obj, ['user', 'tags', 1])",
    "DIG(:obj, ['user', 'nope'], 'fallback')",
    "DIG({\"a\": {\"b\": 1}}, ['a', 'b'])",
    // Null handling
    "SUM(:nil, 1)",
    "SUM(:num_text, :flag)",
    "SUM(:s, 1)",
    "SUM([:num_text, :flag, 1])",
    ":nil == NULL",
    ":nil + 1",
    "CONCAT(:nil, 'x')",
    "LENGTH(:nil)",
    "IF(:nil, 1, 2)",
    // Casts on variables
    ":num_text::Integer + 10",
    ":a::String",
    ":arr::String",
    ":s::Array",
    ":obj::Json",
    ":money::Float",
];

#[test]
fn parity_with_variables() {
    assert_parity(WITH_VARIABLES, ALL_VAR_MODES);
}

// ---------------------------------------------------------------------------
// Corpus: assignments and sequences
// ---------------------------------------------------------------------------

const ASSIGNMENTS: &[&str] = &[
    ":x := 5",
    ":x := 5; :x * 2",
    ":x := 1; :y := :x + 1; :x + :y",
    ":a := :a + 1; :a",
    ":list := [3, 1, 2]; :list.sort()",
    ":o := {\"k\": {\"v\": 9}}; :o.k.v",
    ":o := {\"name\": \"John\"}; :o&.name",
    ":o := {\"name\": \"John\"}; :o&.missing",
    ":nv := NULL; :nv&.anything",
    ":obj2 := {\"user\": {\"posts\": [{\"title\": \"t0\"}, {\"title\": \"t1\"}]}}; DIG(:obj2, ['user', 'posts', 1, 'title'])",
    ":total := SUM(:arr); :total / COUNT(:arr)",
    ":f := :arr.filter(:x > 2); :f.length()",
    ":t := 2; :arr.map(:x * :t)",
    ":z := 1; :z := :z + 1; :z := :z * 10; :z",
];

#[test]
fn parity_assignments() {
    assert_parity(ASSIGNMENTS, &[Mode::Assign, Mode::Context]);
}

// ---------------------------------------------------------------------------
// Corpus: custom Rust functions (local registry)
// ---------------------------------------------------------------------------

const CUSTOM_FUNCTIONS: &[&str] = &[
    "TRIPLE(4)",
    "TRIPLE(:a) + 1",
    "SUM(TRIPLE(1), TRIPLE(2))",
    "GREET('bob')",
    "GREET()",
    "TRIPLE('x')",
    "TRIPLE()",
    "TRIPLE(1, 2)",
    "TRIPLE(TRIPLE(2))",
    "UPPER(GREET('x'))",
    "[...[TRIPLE(1)], TRIPLE(2)]",
    "TRIPLE(...[4])",
];

#[test]
fn parity_custom_functions() {
    assert_parity(CUSTOM_FUNCTIONS, &[Mode::Custom]);
}

// ---------------------------------------------------------------------------
// Corpus: errors — both paths must fail
// ---------------------------------------------------------------------------

const ERRORS: &[&str] = &[
    "UNKNOWNFN(1)",
    "SQRT('abc')",
    "UPPER(1, 2, 3)",
    "SUBSTRING()",
    "[1, 2].upper()",
    "'abc'.nosuchmethod()",
    ":missing_var",
    ":missing_var + 1",
    "FILTER(5, :x > 1)",
    "MAP('str', :x)",
    "{\"a\": 1}.missing",
    "[...NULL]",
    "[...5]",
    "[...'ab']",
];

#[test]
fn parity_errors() {
    assert_parity(ERRORS, ALL_MODES);
}

/// Sanity check on the harness itself: a sample of the corpus must actually evaluate, so the
/// parity tests can't pass vacuously with both sides erroring.
#[test]
fn parity_harness_is_not_vacuous() {
    let vars = fixture_vars();
    for (src, want) in [
        ("2 + 3 * 4", n(14.0)),
        (":arr.filter(:x > 2).sum()", n(12.0)),
        ("COUNTIF(:arr, :x >= 3)", n(3.0)),
    ] {
        let expr = parse(src).unwrap();
        assert_eq!(
            evaluator::eval_with_vars(&expr, &vars).unwrap(),
            want,
            "{}",
            src
        );
        assert_eq!(skillet::evaluate_with(src, &vars).unwrap(), want, "{}", src);
    }
    let reg = local_registry();
    ensure_global_registry();
    let expr = parse("TRIPLE(4)").unwrap();
    assert_eq!(
        evaluator::eval_with_vars_and_custom(&expr, &vars, &reg).unwrap(),
        n(12.0)
    );
    assert_eq!(
        skillet::evaluate_with_custom("TRIPLE(4)", &vars).unwrap(),
        n(12.0)
    );
    let expr = parse("UNKNOWNFN(1)").unwrap();
    assert!(evaluator::eval(&expr).is_err());
    assert!(skillet::evaluate("UNKNOWNFN(1)").is_err());
}

// ---------------------------------------------------------------------------
// Former divergences (BL-001). Each table is one root cause in the removed duplicate
// evaluator (`evaluation/`, deleted per ADR-0010). BL-001b fixed them by keeping the
// evaluator.rs result; the tables are replayed here as regression tests. `KNOWN_DIVERGENCES`
// (pairs skipped by the parity tests above) is empty.
// ---------------------------------------------------------------------------

/// D1: `JQ` and `DIG` were not implemented in `evaluation/`: "Unknown function: JQ" / "DIG".
const DIV_JQ_DIG: &[(&str, Mode)] = &[
    ("JQ(:arguments, '$.accounts[*].amount')", Mode::Vars),
    ("JQ(:arguments, '$.accounts[*].amount')", Mode::Custom),
    ("JQ(:arguments, '$.accounts[*].amount')", Mode::Assign),
    ("JQ(:arguments, '$.accounts[*].amount')", Mode::Context),
    ("SUM(JQ(:arguments, '$.accounts[*].amount'))", Mode::Vars),
    ("SUM(JQ(:arguments, '$.accounts[*].amount'))", Mode::Custom),
    ("SUM(JQ(:arguments, '$.accounts[*].amount'))", Mode::Assign),
    ("SUM(JQ(:arguments, '$.accounts[*].amount'))", Mode::Context),
    ("JQ(:arguments, '$.name')", Mode::Vars),
    ("JQ(:arguments, '$.name')", Mode::Custom),
    ("JQ(:arguments, '$.name')", Mode::Assign),
    ("JQ(:arguments, '$.name')", Mode::Context),
    ("JQ(:obj, '$.items[*].p')", Mode::Vars),
    ("JQ(:obj, '$.items[*].p')", Mode::Custom),
    ("JQ(:obj, '$.items[*].p')", Mode::Assign),
    ("JQ(:obj, '$.items[*].p')", Mode::Context),
    ("DIG(:obj, ['user', 'name'])", Mode::Vars),
    ("DIG(:obj, ['user', 'name'])", Mode::Custom),
    ("DIG(:obj, ['user', 'name'])", Mode::Assign),
    ("DIG(:obj, ['user', 'name'])", Mode::Context),
    ("DIG(:obj, ['user', 'tags', 1])", Mode::Vars),
    ("DIG(:obj, ['user', 'tags', 1])", Mode::Custom),
    ("DIG(:obj, ['user', 'tags', 1])", Mode::Assign),
    ("DIG(:obj, ['user', 'tags', 1])", Mode::Context),
    ("DIG(:obj, ['user', 'nope'], 'fallback')", Mode::Vars),
    ("DIG(:obj, ['user', 'nope'], 'fallback')", Mode::Custom),
    ("DIG(:obj, ['user', 'nope'], 'fallback')", Mode::Assign),
    ("DIG(:obj, ['user', 'nope'], 'fallback')", Mode::Context),
    ("DIG({\"a\": {\"b\": 1}}, ['a', 'b'])", Mode::Vars),
    ("DIG({\"a\": {\"b\": 1}}, ['a', 'b'])", Mode::Custom),
    ("DIG({\"a\": {\"b\": 1}}, ['a', 'b'])", Mode::Assign),
    ("DIG({\"a\": {\"b\": 1}}, ['a', 'b'])", Mode::Context),
    (":obj2 := {\"user\": {\"posts\": [{\"title\": \"t0\"}, {\"title\": \"t1\"}]}}; DIG(:obj2, ['user', 'posts', 1, 'title'])", Mode::Assign),
    (":obj2 := {\"user\": {\"posts\": [{\"title\": \"t0\"}, {\"title\": \"t1\"}]}}; DIG(:obj2, ['user', 'posts', 1, 'title'])", Mode::Context),
];

/// D2: the old `evaluation::eval` (no vars) rejected higher-order builtins with
/// "<NAME> requires variable context"; `evaluator::eval` evaluates them.
const DIV_HOF_WITHOUT_VARS: &[(&str, Mode)] = &[
    ("FILTER([1, 2, 3, 4], :x % 2 == 0)", Mode::Eval),
    ("MAP([1, 2, 3], :x * 10)", Mode::Eval),
    ("REDUCE([1, 2, 3], :acc + :x, 0)", Mode::Eval),
    ("FIND([1, 5, 10], :x > 3)", Mode::Eval),
    ("SUMIF([1, -2, 3], :x > 0)", Mode::Eval),
    ("AVGIF([1, 3, 5, -1], :x > 0)", Mode::Eval),
    ("COUNTIF([1, 2, 3, 4], :x % 2 == 0)", Mode::Eval),
    ("FILTER([1, 2, 3, 4], :v > 2, 'v')", Mode::Eval),
    ("MAP([1, 2, 3], :item + 1, 'item')", Mode::Eval),
    (
        "REDUCE([1, 2, 3], :total + :x, 10, 'x', 'total')",
        Mode::Eval,
    ),
    ("FIND([1, 5, 10], :n > 3, 'n')", Mode::Eval),
];

/// D3: `REDUCE` without explicit param names in `evaluation/` (non-custom path) failed with
/// "Missing variable: :acc": `get_param_name(None)` returned "x" for the accumulator too, so the
/// accumulator was bound to `:x` and `:acc` was never set. The custom-registry path was fine.
const DIV_REDUCE_DEFAULT_ACC: &[(&str, Mode)] = &[
    ("REDUCE([1, 2, 3], :acc + :x, 0)", Mode::Vars),
    ("REDUCE([1, 2, 3], :acc + :x, 0)", Mode::Assign),
    ("REDUCE([1, 2, 3], :acc + :x, 0)", Mode::Context),
    ("REDUCE(:arr, :acc * :x, 1)", Mode::Vars),
    ("REDUCE(:arr, :acc * :x, 1)", Mode::Assign),
    ("REDUCE(:arr, :acc * :x, 1)", Mode::Context),
];

/// D4: the custom-registry higher-order builtins in `evaluation/` ignored the optional parameter
/// name arguments and always bound `:x` / `:acc`: "Undefined variable: v" etc.
const DIV_HOF_CUSTOM_PARAM_NAMES: &[(&str, Mode)] = &[
    ("FILTER([1, 2, 3, 4], :v > 2, 'v')", Mode::Custom),
    ("MAP([1, 2, 3], :item + 1, 'item')", Mode::Custom),
    (
        "REDUCE([1, 2, 3], :total + :x, 10, 'x', 'total')",
        Mode::Custom,
    ),
    ("FIND([1, 5, 10], :n > 3, 'n')", Mode::Custom),
];

/// D5: the old `evaluation::eval` (no-vars entry point) was a reduced evaluator: string/null
/// equality failed with "Comparison on non-number", and property access, `&.` (property and
/// method) and `;` sequences failed with "Use eval_with_vars for ...". `NULL == NULL` is `true`
/// since ADR-0013 (pinned in `tests/null_equality.rs`); here both layers must agree.
const DIV_EVAL_NO_VARS: &[(&str, Mode)] = &[
    ("'a' == 'a'", Mode::Eval),
    ("NULL == NULL", Mode::Eval),
    ("{\"a\": {\"b\": 2}}.a.b", Mode::Eval),
    ("{\"a\": 1}&.a", Mode::Eval),
    ("{\"a\": 1}&.missing", Mode::Eval),
    ("NULL&.anything", Mode::Eval),
    ("NULL&.upper()", Mode::Eval),
    ("'abc'&.upper()", Mode::Eval),
    ("1; 2; 3", Mode::Eval),
];

const KNOWN_DIVERGENCES: &[&[(&str, Mode)]] = &[];

#[test]
fn divergence_eval_without_vars_is_reduced() {
    assert_parity_each(DIV_EVAL_NO_VARS);
}

#[test]
fn divergence_jq_and_dig_missing_in_evaluation() {
    assert_parity_each(DIV_JQ_DIG);
}

#[test]
fn divergence_higher_order_builtins_without_vars() {
    assert_parity_each(DIV_HOF_WITHOUT_VARS);
}

#[test]
fn divergence_reduce_default_acc_param() {
    assert_parity_each(DIV_REDUCE_DEFAULT_ACC);
}

#[test]
fn divergence_higher_order_custom_param_names() {
    assert_parity_each(DIV_HOF_CUSTOM_PARAM_NAMES);
}

// ---------------------------------------------------------------------------
// Golden values (BL-087, QA-1). The parity tests above compare two layers that now share one
// evaluator, so a change in what an expression means passes them. This test pins the public
// layer's outcome (value, or error message) for every (expression, mode) pair of the corpus in
// `tests/golden/evaluator_parity.txt`. A change in meaning fails here and shows up as a diff
// line in review. When the change is intended, rewrite the file and commit it with the change:
//
//     UPDATE_GOLDEN=1 cargo test --test evaluator_parity golden
// ---------------------------------------------------------------------------

const GOLDEN_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/golden/evaluator_parity.txt"
);

/// Every (expression, mode) pair the parity tests evaluate, in a stable order, without repeats.
fn golden_corpus() -> Vec<(&'static str, Mode)> {
    let tables: &[(&[&str], &[Mode])] = &[
        (LITERALS_AND_OPERATORS, ALL_MODES),
        (BUILTINS, ALL_MODES),
        (WITH_VARIABLES, ALL_VAR_MODES),
        (ASSIGNMENTS, &[Mode::Assign, Mode::Context]),
        (CUSTOM_FUNCTIONS, &[Mode::Custom]),
        (ERRORS, ALL_MODES),
    ];
    let divergences: &[&[(&str, Mode)]] = &[
        DIV_JQ_DIG,
        DIV_HOF_WITHOUT_VARS,
        DIV_REDUCE_DEFAULT_ACC,
        DIV_HOF_CUSTOM_PARAM_NAMES,
        DIV_EVAL_NO_VARS,
    ];
    let mut out: Vec<(&'static str, Mode)> = Vec::new();
    let pairs = tables
        .iter()
        .flat_map(|(t, modes)| {
            t.iter()
                .flat_map(move |s| modes.iter().map(move |m| (*s, *m)))
        })
        .chain(divergences.iter().flat_map(|t| t.iter().copied()));
    for (src, mode) in pairs {
        if !out.iter().any(|(s, m)| *s == src && m.tag() == mode.tag()) {
            out.push((src, mode));
        }
    }
    out
}

impl Mode {
    fn tag(self) -> &'static str {
        match self {
            Mode::Eval => "eval",
            Mode::Vars => "vars",
            Mode::Custom => "custom",
            Mode::Assign => "assign",
            Mode::Context => "context",
        }
    }
}

/// One line per pair: `<mode> <expression as a Rust string literal> => <outcome>`.
fn golden_line(src: &str, mode: Mode) -> String {
    let expr = parse(src).unwrap_or_else(|e| panic!("does not parse: {src:?}: {}", e.message));
    let (_, public) = layers(src, &expr, mode);
    let outcome = match public {
        Outcome::Ok(v) => format!("Ok {v}"),
        Outcome::Err(m) => format!("Err {m:?}"),
        Outcome::Panic => "Panic".to_string(),
    };
    format!("{} {:?} => {}", mode.tag(), src, outcome)
}

#[test]
fn golden_values_for_the_whole_corpus() {
    let corpus = golden_corpus();
    let mut actual = String::from(
        "# Golden values for tests/evaluator_parity.rs (BL-087). Generated; do not edit by hand.\n\
         # Rewrite: UPDATE_GOLDEN=1 cargo test --test evaluator_parity golden\n",
    );
    for (src, mode) in &corpus {
        actual.push_str(&golden_line(src, *mode));
        actual.push('\n');
    }
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(std::path::Path::new(GOLDEN_PATH).parent().unwrap()).unwrap();
        std::fs::write(GOLDEN_PATH, &actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(GOLDEN_PATH).unwrap_or_default();
    let want: Vec<&str> = expected.lines().collect();
    let got: Vec<&str> = actual.lines().collect();
    let changed: Vec<String> = got
        .iter()
        .filter(|l| !want.contains(l))
        .map(|l| format!("  now:  {l}"))
        .chain(
            want.iter()
                .filter(|l| !got.contains(l))
                .map(|l| format!("  was:  {l}")),
        )
        .collect();
    assert!(
        changed.is_empty(),
        "{} golden line(s) differ from {GOLDEN_PATH}.\n\
         If the change in meaning is intended, run\n  \
         UPDATE_GOLDEN=1 cargo test --test evaluator_parity golden\n\
         and commit the file with a note on the decision.\n{}",
        changed.len(),
        changed.join("\n")
    );
    assert_eq!(got.len(), want.len(), "line count differs");
}

/// The golden file covers the corpus: it cannot pass because it is empty or stale.
#[test]
fn golden_file_covers_every_pair() {
    let expected = std::fs::read_to_string(GOLDEN_PATH).expect("tests/golden/evaluator_parity.txt");
    let lines = expected.lines().filter(|l| !l.starts_with('#')).count();
    assert_eq!(lines, golden_corpus().len());
    assert!(lines > 500, "{lines}");
}
