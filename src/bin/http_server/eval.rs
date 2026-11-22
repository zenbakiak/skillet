use skillet::Value;
use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::Instant;

use super::auth::check_authentication;
use super::cache::{clear_cache, evaluate_cached, get_cache_stats, CachedResult};
use super::eval_limits::server_limits;
use super::json_keys::{check_json_keys, sanitize_json_key};
use super::parse_cache;
use super::response_size;
use super::stats::ServerStats;
use super::types::{
    CacheStatsResponse, EvalRequest, EvalResponse, HealthResponse, IncludeVariables,
    ParseCacheStatsResponse,
};
use super::utils::{parse_json_body, send_http_error, send_http_response, to_json_exact};

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
    let json = to_json_exact(&response);
    send_http_response(
        stream,
        if response.success { 200 } else { 400 },
        "application/json",
        &json,
    );
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
    let mut include_variables = IncludeVariables::None;

    for param in query.split('&') {
        if let Some((key, value)) = param.split_once('=') {
            let decoded_value = urlencoding::decode(value).unwrap_or_default();
            match key {
                "expr" | "expression" => expression = decoded_value.to_string(),
                "output_json" => output_json = decoded_value == "true",
                "include_variables" => {
                    if decoded_value == "true" {
                        include_variables = IncludeVariables::All;
                    } else if decoded_value == "false" {
                        include_variables = IncludeVariables::None;
                    } else {
                        let vars: Vec<String> = decoded_value
                            .split(',')
                            .map(|s| s.trim())
                            .filter(|s| !s.is_empty())
                            .map(|s| s.strip_prefix(':').unwrap_or(s).to_string())
                            .collect();

                        if vars.is_empty() {
                            include_variables = IncludeVariables::None;
                        } else {
                            include_variables = IncludeVariables::Selected(vars);
                        }
                    }
                }
                _ => {
                    // Treat as variable
                    if let Ok(num) = decoded_value.parse::<f64>() {
                        variables.insert(key.to_string(), serde_json::json!(num));
                    } else if decoded_value == "true" {
                        variables.insert(key.to_string(), serde_json::json!(true));
                    } else if decoded_value == "false" {
                        variables.insert(key.to_string(), serde_json::json!(false));
                    } else {
                        variables.insert(
                            key.to_string(),
                            serde_json::json!(decoded_value.to_string()),
                        );
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
        arguments: if variables.is_empty() {
            None
        } else {
            Some(variables)
        },
        output_json: Some(output_json),
        include_variables: Some(include_variables),
    };

    let response = process_eval_request(eval_request, stats, request_counter);
    let json = to_json_exact(&response);
    send_http_response(
        stream,
        if response.success { 200 } else { 400 },
        "application/json",
        &json,
    );
}

pub fn handle_health(
    stream: &mut TcpStream,
    stats: &ServerStats,
    request: &str,
    server_token: Arc<Option<String>>,
) {
    // Health endpoint doesn't require authentication
    let _ = (request, server_token); // Suppress unused warnings

    let (requests, avg_time) = stats.get_stats();
    let cache_stats = get_cache_stats();
    let response = HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        requests_processed: requests,
        avg_execution_time_ms: avg_time,
        cache_stats: Some(CacheStatsResponse {
            hits: cache_stats.hits,
            misses: cache_stats.misses,
            hit_rate: cache_stats.hit_rate(),
            entries: cache_stats.entries,
            evictions: cache_stats.evictions,
            total_saved_time_ms: cache_stats.total_saved_time_ms,
        }),
        parse_cache: {
            let parse = parse_cache::stats();
            ParseCacheStatsResponse {
                hits: parse.hits,
                misses: parse.misses,
                hit_rate: parse.hit_rate(),
                entries: parse.entries,
            }
        },
    };

    let json = serde_json::to_string(&response).unwrap_or_default();
    send_http_response(stream, 200, "application/json", &json);
}

pub fn handle_cache_clear(
    stream: &mut TcpStream,
    _request: &str,
    server_admin_token: Arc<Option<String>>,
) {
    // Check admin authentication
    if let Some(error_response) = check_authentication(_request, &server_admin_token) {
        send_http_response(stream, 401, "application/json", &error_response);
        return;
    }

    clear_cache();
    // Every worker's parsed trees too: this one's now, the others' at their next request.
    parse_cache::clear_all();

    let response = serde_json::json!({
        "success": true,
        "message": "Expression cache cleared successfully"
    });

    send_http_response(stream, 200, "application/json", &response.to_string());
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
            if let Err(e) = check_json_keys(json_vars.keys().map(String::as_str)) {
                return EvalResponse {
                    success: false,
                    result: None,
                    variables: None,
                    error: Some(e),
                    execution_time_ms: start_time.elapsed().as_secs_f64() * 1000.0,
                    request_id,
                };
            }
            // Pre-allocate HashMap with known size + 1 for "arguments"
            let mut result = HashMap::with_capacity(json_vars.len() + 1);

            // The original JSON data for the JQ function (serialize once)
            let json_str = serde_json::to_string(&json_vars).unwrap_or_default();

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
            // Inserted last so a request key named `arguments` cannot replace it (BL-077).
            result.insert("arguments".to_string(), Value::Json(json_str));
            result
        }
        None => HashMap::new(),
    };

    // Evaluate expression with caching
    let include_variables = matches!(
        req.include_variables,
        Some(IncludeVariables::All) | Some(IncludeVariables::Selected(_))
    );
    // The variables are charged in the evaluation's own budget scope (ADR-0011 amendment 5):
    // `evaluate_cached` opens the same limits again, which joins this scope.
    let cached_result =
        skillet::with_limits(server_limits(), || match skillet::charge_variables(&vars) {
            Ok(()) => evaluate_cached(&req.expression, &vars, include_variables),
            Err(e) => CachedResult {
                result: Err(e.to_string()),
                variable_context: None,
            },
        });

    let (result, variable_context) = match cached_result.result {
        Ok(value) => (Ok(value), cached_result.variable_context),
        Err(error_msg) => (Err(skillet::Error::new(error_msg, None)), None),
    };

    let execution_time = start_time.elapsed();
    let execution_time_ms = execution_time.as_secs_f64() * 1000.0;
    stats.record_request(execution_time.as_micros() as u64);

    let failed = |error: String| EvalResponse {
        success: false,
        result: None,
        variables: None,
        error: Some(error),
        execution_time_ms,
        request_id,
    };

    match result {
        Ok(val) => {
            // Size the response before building it: escaping can make it up to 6x the value.
            let mut response_bytes = response_size::json_size(&val, false);
            if let Err(e) = response_size::check(response_bytes, server_limits().max_bytes) {
                return failed(e);
            }
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
                        // Apply selective filtering based on include_variables
                        let should_include = match &req.include_variables {
                            Some(IncludeVariables::All) => true,
                            Some(IncludeVariables::Selected(selected_vars)) => {
                                selected_vars.contains(&key)
                            }
                            _ => false,
                        };

                        if should_include {
                            response_bytes = response_bytes
                                .saturating_add(key.len() as u64 + 4)
                                .saturating_add(response_size::json_size(&value, false));
                            if let Err(e) =
                                response_size::check(response_bytes, server_limits().max_bytes)
                            {
                                return failed(e);
                            }
                            json_vars.insert(key, format_simple_output(&value));
                        }
                    }
                }
                if json_vars.is_empty() {
                    None
                } else {
                    Some(json_vars)
                }
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
        Err(e) => failed(e.to_string()),
    }
}

#[allow(deprecated)] // `Value::Currency` is read as a Number (ADR-0033)
fn format_structured_output(val: &Value, execution_time_ms: f64) -> serde_json::Value {
    let (result_value, type_name) = match val {
        Value::Number(n) | Value::Currency(n) => (serde_json::json!(n), "Number"),
        Value::String(s) => (serde_json::json!(s), "String"),
        Value::Boolean(b) => (serde_json::json!(b), "Boolean"),
        Value::DateTime(dt) => (serde_json::json!(dt), "DateTime"),
        Value::Array(arr) => {
            let json_arr: Vec<serde_json::Value> = arr.iter().map(format_simple_output).collect();
            (serde_json::json!(json_arr), "Array")
        }
        Value::Null => (serde_json::json!(null), "Null"),
        Value::Json(s) => match serde_json::from_str(s) {
            Ok(parsed) => (parsed, "Json"),
            Err(_) => (serde_json::json!(s), "Json"),
        },
    };

    serde_json::json!({
        "result": result_value,
        "type": type_name,
        "execution_time": format!("{:.2} ms", execution_time_ms)
    })
}

#[allow(deprecated)] // `Value::Currency` is read as a Number (ADR-0033)
fn format_simple_output(val: &Value) -> serde_json::Value {
    match val {
        Value::Number(n) | Value::Currency(n) => serde_json::json!(n),
        Value::String(s) => serde_json::json!(s),
        Value::Boolean(b) => serde_json::json!(b),
        Value::DateTime(dt) => serde_json::json!(dt.to_string()),
        Value::Array(arr) => {
            let json_arr: Vec<serde_json::Value> = arr.iter().map(format_simple_output).collect();
            serde_json::json!(json_arr)
        }
        Value::Null => serde_json::json!(null),
        Value::Json(s) => serde_json::from_str(s).unwrap_or_else(|_| serde_json::json!(s)),
    }
}
