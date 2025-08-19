use skillet::{evaluate_with_custom, Value, JSPluginLoader};
use std::collections::HashMap;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    
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
        eprintln!("Usage: sk \"=SUM(:sales, 1000)\" [var=value ...] OR sk \"=SUM(:sales, 1000)\" --json '{{\"sales\": 5000}}'");
        eprintln!("Examples:");
        eprintln!("  # Key-value variables:");
        eprintln!("  sk \"=2 + 3 * 4\"");
        eprintln!("  sk \"=SUM(:sales, 1000)\" sales=5000");
        eprintln!("  sk \"=:name.upper()\" name=\"hello world\"");
        eprintln!("  sk \"=:price * :quantity\" price=19.99 quantity=3");
        eprintln!("");
        eprintln!("  # JSON variables:");
        eprintln!("  sk \"=SUM(:sales, :bonus)\" --json '{{\"sales\": 5000, \"bonus\": 1000}}'");
        eprintln!("  sk \"=:user.name.upper()\" --json '{{\"user\": {{\"name\": \"alice\"}}}}'");
        eprintln!("  sk \"=:numbers.length()\" --json '{{\"numbers\": [1, 2, 3, 4, 5]}}'");
        std::process::exit(1);
    }

    // Check if we're using JSON mode
    if args.len() >= 3 && args[1] == "--json" {
        let expr = &args[0];
        let json_str = &args[2];
        
        let result = skillet::evaluate_with_json_custom(expr, json_str);
        match result {
            Ok(val) => println!("{:?}", val),
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(2);
            }
        }
        return;
    }

    // Original key-value mode
    let expr = &args[0];
    
    // Parse variable assignments from remaining arguments
    let mut vars = HashMap::new();
    for arg in &args[1..] {
        if arg == "--json" {
            eprintln!("Error: --json flag requires expression and JSON string");
            eprintln!("Usage: sk \"expression\" --json '{{\"var\": \"value\"}}'");
            std::process::exit(1);
        }
        
        if let Some((name, value_str)) = arg.split_once('=') {
            let value = parse_value(value_str);
            vars.insert(name.to_string(), value);
        } else {
            eprintln!("Invalid variable assignment: '{}'. Use format: var=value", arg);
            std::process::exit(1);
        }
    }

    // Always use evaluate_with_custom to properly handle variables and custom functions
    let result = evaluate_with_custom(expr, &vars);

    match result {
        Ok(val) => println!("{:?}", val),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(2);
        }
    }
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

