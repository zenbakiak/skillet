use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use std::time::Instant;
use skillet::{evaluate_with_custom, evaluate_with_assignments, evaluate_with_assignments_and_context, Value};

use super::auth::check_authentication;
use super::types::{EvalRequest, EvalResponse, HealthResponse};
use super::utils::{send_http_response, send_http_error, parse_json_body, sanitize_json_key};
use super::stats::ServerStats;

pub fn handle_eval_post(
    stream: &mut TcpStream,
    request: &str,
    stats: Arc<ServerStats>,
    request_counter: Arc<AtomicU64>,
    server_token: Arc<Option<String>>,
) {
    // Check authentication first
    if let Some(error_response) = check_authentication(request, &server_token) {
        send_http_response(stream, 401, "application/json", &error_response);
        return;
    }

    let eval_request: EvalRequest = match parse_json_body(request) {
        Ok(req) => req,
        Err(e) => {
            send_http_error(stream, 400, &e);
            return;
        }
    };

    let response = process_eval_request(eval_request, stats, request_counter);
    let json = serde_json::to_string(&response).unwrap_or_default();
    send_http_response(stream, if response.success { 200 } else { 400 }, "application/json", &json);
}

pub fn handle_eval_get(
    stream: &mut TcpStream,
    request: &str,
    stats: Arc<ServerStats>,
    request_counter: Arc<AtomicU64>,
    server_token: Arc<Option<String>>,
) {
    // Check authentication first
    if let Some(error_response) = check_authentication(request, &server_token) {
        send_http_response(stream, 401, "application/json", &error_response);
        return;
    }

    let request_line = request.lines().next().unwrap_or("");
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        send_http_error(stream, 400, "Bad Request");
        return;
    }

    let path_and_query = parts[1];
    let (_, query) = path_and_query.split_once('?').unwrap_or(("", ""));

    // Parse query parameters
    let mut expression = String::new();
    let mut variables = HashMap::new();
    let mut output_json = false;
    let mut include_variables = false;

    for param in query.split('&') {
        if let Some((key, value)) = param.split_once('=') {
            let decoded_value = urlencoding::decode(value).unwrap_or_default();
            match key {
                "expr" | "expression" => expression = decoded_value.to_string(),
                "output_json" => output_json = decoded_value == "true",
                "include_variables" => include_variables = decoded_value == "true",
                _ => {
                    // Treat as variable
                    if let Ok(num) = decoded_value.parse::<f64>() {
                        variables.insert(key.to_string(), serde_json::json!(num));
                    } else if decoded_value == "true" {
                        variables.insert(key.to_string(), serde_json::json!(true));
                    } else if decoded_value == "false" {
                        variables.insert(key.to_string(), serde_json::json!(false));
                    } else {
                        variables.insert(key.to_string(), serde_json::json!(decoded_value.to_string()));
                    }
                }
            }
        }
    }

    if expression.is_empty() {
        send_http_error(stream, 400, "Missing expression parameter");
        return;
    }

    let eval_request = EvalRequest {
        expression,
        arguments: if variables.is_empty() { None } else { Some(variables) },
        output_json: Some(output_json),
        include_variables: Some(include_variables),
    };

    let response = process_eval_request(eval_request, stats, request_counter);
    let json = serde_json::to_string(&response).unwrap_or_default();
    send_http_response(stream, if response.success { 200 } else { 400 }, "application/json", &json);
}

pub fn handle_health(
    stream: &mut TcpStream,
    stats: &ServerStats,
    request: &str,
    server_token: Arc<Option<String>>
) {
    // Health endpoint doesn't require authentication
    let _ = (request, server_token); // Suppress unused warnings

    let (requests, avg_time) = stats.get_stats();
    let response = HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        requests_processed: requests,
        avg_execution_time_ms: avg_time,
    };

    let json = serde_json::to_string(&response).unwrap_or_default();
    send_http_response(stream, 200, "application/json", &json);
}

fn process_eval_request(
    req: EvalRequest,
    stats: Arc<ServerStats>,
    request_counter: Arc<AtomicU64>,
) -> EvalResponse {
    let request_id = request_counter.fetch_add(1, Ordering::Relaxed);
    let start_time = Instant::now();

    // Convert JSON variables to Skillet values with key sanitization
    let vars = match req.arguments {
        Some(json_vars) => {
            let mut result = HashMap::new();
            for (key, value) in json_vars {
                match skillet::json_to_value(value) {
                    Ok(v) => {
                        let sanitized_key = sanitize_json_key(&key);
                        result.insert(sanitized_key, v);
                    }
                    Err(e) => {
                        return EvalResponse {
                            success: false,
                            result: None,
                            variables: None,
                            error: Some(format!("Error converting variable '{}': {}", key, e)),
                            execution_time_ms: start_time.elapsed().as_secs_f64() * 1000.0,
                            request_id,
                        };
                    }
                }
            }
            result
        }
        None => HashMap::new(),
    };

    // Evaluate expression
    let (result, variable_context) = if req.expression.contains(";") || req.expression.contains(":=") {
        // Use the new function that returns both result and variable context
        if req.include_variables.unwrap_or(false) {
            match evaluate_with_assignments_and_context(&req.expression, &vars) {
                Ok((val, ctx)) => (Ok(val), Some(ctx)),
                Err(e) => (Err(e), None),
            }
        } else {
            (evaluate_with_assignments(&req.expression, &vars), None)
        }
    } else {
        (evaluate_with_custom(&req.expression, &vars), None)
    };

    let execution_time = start_time.elapsed();
    let execution_time_ms = execution_time.as_secs_f64() * 1000.0;
    stats.record_request(execution_time.as_micros() as u64);

    match result {
        Ok(val) => {
            let result_json = if req.output_json.unwrap_or(false) {
                format_structured_output(&val, execution_time_ms)
            } else {
                format_simple_output(&val)
            };

            // Convert variable context to JSON if requested
            let variables_json = if let Some(ctx) = variable_context {
                let mut json_vars = HashMap::new();
                for (key, value) in ctx {
                    // Include all variables that were assigned during evaluation
                    // Skip initial arguments that haven't changed
                    if !vars.contains_key(&key) || vars.get(&key) != Some(&value) {
                        json_vars.insert(key, format_simple_output(&value));
                    }
                }
                if json_vars.is_empty() { None } else { Some(json_vars) }
            } else {
                None
            };

            EvalResponse {
                success: true,
                result: Some(result_json),
                variables: variables_json,
                error: None,
                execution_time_ms,
                request_id,
            }
        }
        Err(e) => EvalResponse {
            success: false,
            result: None,
            variables: None,
            error: Some(e.to_string()),
            execution_time_ms,
            request_id,
        },
    }
}

fn format_structured_output(val: &Value, execution_time_ms: f64) -> serde_json::Value {
    let (result_value, type_name) = match val {
        Value::Number(n) => (serde_json::json!(n), "Number"),
        Value::String(s) => (serde_json::json!(s), "String"),
        Value::Boolean(b) => (serde_json::json!(b), "Boolean"),
        Value::Currency(c) => (serde_json::json!(c), "Currency"),
        Value::DateTime(dt) => (serde_json::json!(dt), "DateTime"),
        Value::Array(arr) => {
            let json_arr: Vec<serde_json::Value> = arr.iter().map(format_simple_output).collect();
            (serde_json::json!(json_arr), "Array")
        },
        Value::Null => (serde_json::json!(null), "Null"),
        Value::Json(s) => {
            match serde_json::from_str(s) {
                Ok(parsed) => (parsed, "Json"),
                Err(_) => (serde_json::json!(s), "Json")
            }
        }
    };

    serde_json::json!({
        "result": result_value,
        "type": type_name,
        "execution_time": format!("{:.2} ms", execution_time_ms)
    })
}

fn format_simple_output(val: &Value) -> serde_json::Value {
    match val {
        Value::Number(n) => serde_json::json!(n),
        Value::String(s) => serde_json::json!(s),
        Value::Boolean(b) => serde_json::json!(b),
        Value::Currency(c) => serde_json::json!(c),
        Value::DateTime(dt) => serde_json::json!(dt.to_string()),
        Value::Array(arr) => {
            let json_arr: Vec<serde_json::Value> = arr.iter().map(format_simple_output).collect();
            serde_json::json!(json_arr)
        },
        Value::Null => serde_json::json!(null),
        Value::Json(s) => serde_json::from_str(s).unwrap_or_else(|_| serde_json::json!(s)),
    }
}