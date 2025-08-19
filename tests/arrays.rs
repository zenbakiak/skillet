use skillet::{evaluate, Value};

fn s(v: Value) -> String { if let Value::String(s) = v { s } else { panic!("expected string") } }
fn b(v: Value) -> bool { if let Value::Boolean(b) = v { b } else { panic!("expected bool") } }

#[test]
fn array_builtins() {
    // ARRAY construction
    match evaluate("ARRAY(1, 2, 3)").unwrap() { Value::Array(v) => assert_eq!(v.len(), 3), _ => panic!() }
    // FIRST, LAST
    assert!(matches!(evaluate("FIRST([9,8,7])").unwrap(), Value::Number(9.0)));
    assert!(matches!(evaluate("LAST([9,8,7])").unwrap(), Value::Number(7.0)));
    // CONTAINS
    assert!(b(evaluate("CONTAINS([1,2,3], 2)").unwrap()));
    // UNIQUE
    match evaluate("UNIQUE([1,2,2,3])").unwrap() { Value::Array(v) => assert_eq!(v, vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]), _ => panic!() }
    // SORT and REVERSE
    match evaluate("SORT([3,1,2])").unwrap() { Value::Array(v) => assert_eq!(v, vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]), _ => panic!() }
    match evaluate("SORT([3,1,2], 'DESC')").unwrap() { Value::Array(v) => assert_eq!(v, vec![Value::Number(3.0), Value::Number(2.0), Value::Number(1.0)]), _ => panic!() }
    match evaluate("REVERSE([1,2,3])").unwrap() { Value::Array(v) => assert_eq!(v, vec![Value::Number(3.0), Value::Number(2.0), Value::Number(1.0)]), _ => panic!() }
    // JOIN
    assert_eq!(s(evaluate("JOIN([1,2,3], '-')").unwrap()), "1-2-3");
}

#[test]
fn spread_and_filter_map_reduce() {
    use Value::*;
    // Spread in SUM and CONCAT
    assert!(matches!(evaluate("SUM(...[1,2,3])").unwrap(), Number(6.0)));
    assert_eq!(s(evaluate("CONCAT(...['a','b','c'])").unwrap()), "abc");
    // filter/map chain: [30,60,80,100].filter(:x > 50).map(:x * 0.9).sum()
    assert!(matches!(evaluate("[30,60,80,100].filter(:x > 50).map(:x * 0.9).sum() ").unwrap(), Number(n) if (n-216.0).abs()<1e-9));
    // reduce: sum with initial 0
    assert!(matches!(evaluate("[1,2,3].reduce(:acc + :x, 0)").unwrap(), Number(6.0)));
    // Function forms
    assert!(matches!(evaluate("FILTER([1,2,3,4], :x % 2 == 0)").unwrap(), Value::Array(v) if v == vec![Number(2.0), Number(4.0)]));
    assert!(matches!(evaluate("MAP([1,2,3], :x * 10)").unwrap(), Value::Array(v) if v == vec![Number(10.0), Number(20.0), Number(30.0)]));
    assert!(matches!(evaluate("REDUCE([1,2,3], :acc + :x, 0)").unwrap(), Number(6.0)));
}

#[test]
fn sumif_avgif_countif_flatten() {
    use Value::*;
    assert!(matches!(evaluate("SUMIF([1, -2, 3, -4], :x > 0)").unwrap(), Number(4.0)));
    assert!(matches!(evaluate("AVGIF([1, 3, 5, -1], :x > 0)").unwrap(), Number(n) if (n-3.0).abs()<1e-9));
    assert!(matches!(evaluate("COUNTIF([1,2,3,4], :x % 2 == 0)").unwrap(), Number(2.0)));
    match evaluate("FLATTEN([1,[2,[3]],4])").unwrap() { Value::Array(v) => assert_eq!(v, vec![Number(1.0), Number(2.0), Number(3.0), Number(4.0)]), _ => panic!() }
    match evaluate("[1,[2,[3]],4].flatten()").unwrap() { Value::Array(v) => assert_eq!(v, vec![Number(1.0), Number(2.0), Number(3.0), Number(4.0)]), _ => panic!() }
}
