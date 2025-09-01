use skillet::{evaluate_with_custom, evaluate_with_assignments, Value, JSPluginLoader};
use std::collections::HashMap;
use std::time::Instant;
use serde_json::json;

/// Sanitize JSON keys by replacing special characters with underscores
fn sanitize_json_key(key: &str) -> String {
    key.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    
    // Register built-in Rust functions (none by default)
    
    // Auto-load JavaScript functions from hooks directory
    let hooks_dir = std::env::var("SKILLET_HOOKS_DIR").unwrap_or_else(|_| "hooks".to_string());
    let js_loader = JSPluginLoader::new(hooks_dir);
    
    match js_loader.auto_register() {
        Ok(count) => {
            if count > 0 {
                eprintln!("Loaded {} custom JavaScript function(s)", count);
            }
        }
        Err(e) => {
            eprintln!("Warning: Failed to load JavaScript functions: {}", e);
        }
    }
    
    if args.is_empty() {
        eprintln!("Usage: sk \"expression\" [options] [var=value ...]");
        eprintln!("       sk \"expression\" --json '{{\"var\": \"value\"}}'");
        eprintln!("");
        eprintln!("Options:");
        eprintln!("  --output-json    Output result in JSON format with type and timing");
        eprintln!("  --json JSON      Use JSON string for variable values");
        eprintln!("");
        eprintln!("Examples:");
        eprintln!("  # Basic usage:");
        eprintln!("  sk \"=2 + 3 * 4\"");
        eprintln!("  sk \"=2 + 3 * 4\" --output-json");
        eprintln!("");
        eprintln!("  # Key-value variables:");
        eprintln!("  sk \"=SUM(:sales, 1000)\" sales=5000");
        eprintln!("  sk \"=:name.upper()\" name=\"hello world\" --output-json");
        eprintln!("  sk \"=:price * :quantity\" price=19.99 quantity=3");
        eprintln!("");
        eprintln!("  # JSON variables:");
        eprintln!("  sk \"=SUM(:sales, :bonus)\" --json '{{\"sales\": 5000, \"bonus\": 1000}}'");
        eprintln!("  sk \"=:user.name.upper()\" --json '{{\"user\": {{\"name\": \"alice\"}}}}' --output-json");
        eprintln!("  sk \"=:numbers.length()\" --json '{{\"numbers\": [1, 2, 3, 4, 5]}}'");
        std::process::exit(1);
    }

    // Parse arguments and flags
    let mut expr = "";
    let mut json_input = None;
    let mut output_json = false;
    let mut vars = HashMap::new();
    let mut i = 0;
    
    while i < args.len() {
        let arg = &args[i];
        
        if i == 0 {
            // First argument is always the expression
            expr = arg;
        } else if arg == "--json" {
            // --json flag requires a JSON string argument
            if i + 1 >= args.len() {
                eprintln!("Error: --json flag requires a JSON string argument");
                eprintln!("Usage: sk \"expression\" --json '{{\"var\": \"value\"}}'");
                std::process::exit(1);
            }
            json_input = Some(args[i + 1].clone());
            i += 1; // Skip the JSON string argument
        } else if arg == "--output-json" {
            output_json = true;
        } else if let Some((name, value_str)) = arg.split_once('=') {
            // Variable assignment
            let value = parse_value(value_str);
            vars.insert(name.to_string(), value);
        } else {
            eprintln!("Invalid variable assignment: '{}'. Use format: var=value", arg);
            std::process::exit(1);
        }
        
        i += 1;
    }
    
    // Measure execution time
    let start_time = Instant::now();
    
    let result = if let Some(json_str) = json_input {
        // For JSON input, first check if expression contains assignments/sequences
        if expr.contains(";") || expr.contains(":=") {
            // Need to parse JSON and pass to assignment evaluator
            let json_value: serde_json::Value = match serde_json::from_str(&json_str) {
                Ok(val) => val,
                Err(e) => {
                    eprintln!("Error: Invalid JSON: {}", e);
                    std::process::exit(1);
                }
            };
            let vars = match json_value {
                serde_json::Value::Object(map) => {
                    let mut result = HashMap::new();
                    for (key, value) in map {
                        let skillet_value = match skillet::json_to_value(value) {
                            Ok(v) => v,
                            Err(e) => {
                                eprintln!("Error converting JSON: {}", e);
                                std::process::exit(1);
                            }
                        };
                        let sanitized_key = sanitize_json_key(&key);
                        result.insert(sanitized_key, skillet_value);
                    }
                    result
                }
                _ => {
                    eprintln!("Error: JSON must be an object with key-value pairs");
                    std::process::exit(1);
                }
            };
            evaluate_with_assignments(expr, &vars)
        } else {
            skillet::evaluate_with_json_custom(expr, &json_str)
        }
    } else if expr.contains(";") || expr.contains(":=") {
        evaluate_with_assignments(expr, &vars)
    } else {
        evaluate_with_custom(expr, &vars)
    };
    
    let execution_time = start_time.elapsed();
    let execution_time_ms = execution_time.as_secs_f64() * 1000.0;

    match result {
        Ok(val) => {
            if output_json {
                println!("{}", format_json_output(&val, execution_time_ms));
            } else {
                println!("{:?}", val);
            }
        },
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(2);
        }
    }
}

fn format_json_output(value: &Value, execution_time_ms: f64) -> String {
    let (result_value, type_name) = match value {
        Value::Number(n) => (json!(n), "Number"),
        Value::String(s) => (json!(s), "String"),
        Value::Boolean(b) => (json!(b), "Boolean"),
        Value::Currency(c) => (json!(c), "Currency"),
        Value::DateTime(dt) => (json!(dt), "DateTime"),
        Value::Array(arr) => {
            let json_arr: Vec<serde_json::Value> = arr.iter().map(|v| match v {
                Value::Number(n) => json!(n),
                Value::String(s) => json!(s),
                Value::Boolean(b) => json!(b),
                Value::Currency(c) => json!(c),
                Value::DateTime(dt) => json!(dt),
                Value::Null => json!(null),
                Value::Array(_) => json!(format!("{:?}", v)), // Nested arrays as debug string for now
                Value::Json(s) => serde_json::from_str(s).unwrap_or_else(|_| json!(s)),
            }).collect();
            (json!(json_arr), "Array")
        },
        Value::Null => (json!(null), "Null"),
        Value::Json(s) => {
            match serde_json::from_str(s) {
                Ok(parsed) => (parsed, "Json"),
                Err(_) => (json!(s), "Json")
            }
        }
    };
    
    let output = json!({
        "result": result_value,
        "type": type_name,
        "execution_time": format!("{:.2} ms", execution_time_ms)
    });
    
    serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
}

fn parse_value(s: &str) -> Value {
    // Try to parse as different types
    
    // Check for string (quoted)
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        return Value::String(s[1..s.len()-1].to_string());
    }
    
    // Check for boolean
    match s.to_lowercase().as_str() {
        "true" => return Value::Boolean(true),
        "false" => return Value::Boolean(false),
        "null" => return Value::Null,
        _ => {}
    }
    
    // Check for array (basic support for [1,2,3] format)
    if s.starts_with('[') && s.ends_with(']') {
        let inner = &s[1..s.len()-1];
        if inner.trim().is_empty() {
            return Value::Array(vec![]);
        }
        let items: Vec<Value> = inner.split(',')
            .map(|item| parse_value(item.trim()))
            .collect();
        return Value::Array(items);
    }
    
    // Try to parse as number
    if let Ok(num) = s.parse::<f64>() {
        return Value::Number(num);
    }
    
    // Default to string if nothing else matches
    Value::String(s.to_string())
}
