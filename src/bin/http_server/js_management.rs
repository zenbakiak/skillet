use std::net::TcpStream;
use std::sync::Arc;
use std::fs;
use skillet::{JSPluginLoader, CustomFunction, Value};
use skillet::js_plugin::JavaScriptFunction;

use super::auth::check_admin_authentication;
use super::types::*;
use super::utils::{send_http_response, send_http_error, parse_json_body};
use super::multipart::{is_multipart_request, extract_boundary_from_content_type, parse_multipart_data};

pub fn handle_list_js(
    stream: &mut TcpStream,
    request: &str,
    server_admin_token: Arc<Option<String>>,
) {
    // Check admin authentication first
    if let Some(error_response) = check_admin_authentication(request, &server_admin_token) {
        send_http_response(stream, 401, "application/json", &error_response);
        return;
    }

    let hooks_dir = std::env::var("SKILLET_HOOKS_DIR").unwrap_or_else(|_| "hooks".to_string());
    
    match list_js_functions(&hooks_dir) {
        Ok(functions) => {
            let response = ListJSResponse {
                success: true,
                total_count: functions.len(),
                functions,
                error: None,
            };
            let json = serde_json::to_string(&response).unwrap_or_default();
            send_http_response(stream, 200, "application/json", &json);
        }
        Err(e) => {
            let response = ListJSResponse {
                success: false,
                functions: Vec::new(),
                total_count: 0,
                error: Some(e),
            };
            let json = serde_json::to_string(&response).unwrap_or_default();
            send_http_response(stream, 500, "application/json", &json);
        }
    }
}

pub fn handle_update_js(
    stream: &mut TcpStream,
    request: &str,
    server_admin_token: Arc<Option<String>>,
) {
    // Check admin authentication first
    if let Some(error_response) = check_admin_authentication(request, &server_admin_token) {
        send_http_response(stream, 401, "application/json", &error_response);
        return;
    }

    let update_request: UpdateJSRequest = match parse_update_request(request) {
        Ok(req) => req,
        Err(e) => {
            send_http_error(stream, 400, &e);
            return;
        }
    };

    // Validate filename ends with .js
    if !update_request.filename.ends_with(".js") {
        let response = UpdateJSResponse {
            success: false,
            message: "Filename must end with .js extension".to_string(),
            function_name: None,
            validation_results: None,
            error: Some("Invalid file extension".to_string()),
        };
        let json = serde_json::to_string(&response).unwrap_or_default();
        send_http_response(stream, 400, "application/json", &json);
        return;
    }

    let hooks_dir = std::env::var("SKILLET_HOOKS_DIR").unwrap_or_else(|_| "hooks".to_string());
    
    // Check if file exists
    let file_path = std::path::Path::new(&hooks_dir).join(&update_request.filename);
    if !file_path.exists() {
        let response = UpdateJSResponse {
            success: false,
            message: format!("File '{}' does not exist", update_request.filename),
            function_name: None,
            validation_results: None,
            error: Some("File not found".to_string()),
        };
        let json = serde_json::to_string(&response).unwrap_or_default();
        send_http_response(stream, 404, "application/json", &json);
        return;
    }

    // Extract JS code from either js_code field or file_content field
    let js_code = match (update_request.js_code.as_ref(), update_request.file_content.as_ref()) {
        (Some(code), _) => code.clone(),
        (None, Some(content)) => content.clone(),
        (None, None) => {
            let response = UpdateJSResponse {
                success: false,
                message: "Either js_code or file_content must be provided".to_string(),
                function_name: None,
                validation_results: None,
                error: Some("Missing JS content".to_string()),
            };
            let json = serde_json::to_string(&response).unwrap_or_default();
            send_http_response(stream, 400, "application/json", &json);
            return;
        }
    };

    // Validate and process the JS function
    match validate_js_function(&js_code) {
        Ok((js_func, validation_results)) => {
            // Update file in hooks directory
            match save_js_file(&hooks_dir, &update_request.filename, &js_code) {
                Ok(_) => {
                    let response = UpdateJSResponse {
                        success: true,
                        message: format!("JavaScript function '{}' updated successfully", js_func.name()),
                        function_name: Some(js_func.name().to_string()),
                        validation_results: Some(validation_results),
                        error: None,
                    };
                    let json = serde_json::to_string(&response).unwrap_or_default();
                    send_http_response(stream, 200, "application/json", &json);
                }
                Err(e) => {
                    let response = UpdateJSResponse {
                        success: false,
                        message: "Validation passed but failed to update file".to_string(),
                        function_name: Some(js_func.name().to_string()),
                        validation_results: Some(validation_results),
                        error: Some(e),
                    };
                    let json = serde_json::to_string(&response).unwrap_or_default();
                    send_http_response(stream, 500, "application/json", &json);
                }
            }
        }
        Err(e) => {
            let response = UpdateJSResponse {
                success: false,
                message: "JavaScript function validation failed".to_string(),
                function_name: None,
                validation_results: None,
                error: Some(e),
            };
            let json = serde_json::to_string(&response).unwrap_or_default();
            send_http_response(stream, 400, "application/json", &json);
        }
    }
}

pub fn handle_delete_js(
    stream: &mut TcpStream,
    request: &str,
    server_admin_token: Arc<Option<String>>,
) {
    // Check admin authentication first
    if let Some(error_response) = check_admin_authentication(request, &server_admin_token) {
        send_http_response(stream, 401, "application/json", &error_response);
        return;
    }

    let delete_request: DeleteJSRequest = match parse_json_body(request) {
        Ok(req) => req,
        Err(e) => {
            send_http_error(stream, 400, &e);
            return;
        }
    };

    // Validate filename ends with .js
    if !delete_request.filename.ends_with(".js") {
        let response = DeleteJSResponse {
            success: false,
            message: "Filename must end with .js extension".to_string(),
            error: Some("Invalid file extension".to_string()),
        };
        let json = serde_json::to_string(&response).unwrap_or_default();
        send_http_response(stream, 400, "application/json", &json);
        return;
    }

    let hooks_dir = std::env::var("SKILLET_HOOKS_DIR").unwrap_or_else(|_| "hooks".to_string());
    
    match delete_js_file(&hooks_dir, &delete_request.filename) {
        Ok(_) => {
            let response = DeleteJSResponse {
                success: true,
                message: format!("JavaScript function file '{}' deleted successfully", delete_request.filename),
                error: None,
            };
            let json = serde_json::to_string(&response).unwrap_or_default();
            send_http_response(stream, 200, "application/json", &json);
        }
        Err(e) => {
            let response = DeleteJSResponse {
                success: false,
                message: format!("Failed to delete file '{}'", delete_request.filename),
                error: Some(e),
            };
            let json = serde_json::to_string(&response).unwrap_or_default();
            send_http_response(stream, 500, "application/json", &json);
        }
    }
}

pub fn handle_upload_js(
    stream: &mut TcpStream,
    request: &str,
    server_admin_token: Arc<Option<String>>,
) {
    // Check admin authentication first
    if let Some(error_response) = check_admin_authentication(request, &server_admin_token) {
        send_http_response(stream, 401, "application/json", &error_response);
        return;
    }

    let upload_request: UploadJSRequest = match parse_upload_request(request) {
        Ok(req) => req,
        Err(e) => {
            send_http_error(stream, 400, &e);
            return;
        }
    };

    // Validate filename ends with .js
    if !upload_request.filename.ends_with(".js") {
        let response = UploadJSResponse {
            success: false,
            message: "Filename must end with .js extension".to_string(),
            function_name: None,
            validation_results: None,
            error: Some("Invalid file extension".to_string()),
        };
        let json = serde_json::to_string(&response).unwrap_or_default();
        send_http_response(stream, 400, "application/json", &json);
        return;
    }

    // Extract JS code from either js_code field or file_content field
    let js_code = match (upload_request.js_code.as_ref(), upload_request.file_content.as_ref()) {
        (Some(code), _) => code.clone(),
        (None, Some(content)) => content.clone(),
        (None, None) => {
            let response = UploadJSResponse {
                success: false,
                message: "Either js_code or file_content must be provided".to_string(),
                function_name: None,
                validation_results: None,
                error: Some("Missing JS content".to_string()),
            };
            let json = serde_json::to_string(&response).unwrap_or_default();
            send_http_response(stream, 400, "application/json", &json);
            return;
        }
    };

    // Validate and process the JS function
    match validate_js_function(&js_code) {
        Ok((js_func, validation_results)) => {
            // Save file to hooks directory
            let hooks_dir = std::env::var("SKILLET_HOOKS_DIR").unwrap_or_else(|_| "hooks".to_string());
            match save_js_file(&hooks_dir, &upload_request.filename, &js_code) {
                Ok(_) => {
                    let response = UploadJSResponse {
                        success: true,
                        message: format!("JavaScript function '{}' uploaded and validated successfully", js_func.name()),
                        function_name: Some(js_func.name().to_string()),
                        validation_results: Some(validation_results),
                        error: None,
                    };
                    let json = serde_json::to_string(&response).unwrap_or_default();
                    send_http_response(stream, 200, "application/json", &json);
                }
                Err(e) => {
                    let response = UploadJSResponse {
                        success: false,
                        message: "Validation passed but failed to save file".to_string(),
                        function_name: Some(js_func.name().to_string()),
                        validation_results: Some(validation_results),
                        error: Some(e),
                    };
                    let json = serde_json::to_string(&response).unwrap_or_default();
                    send_http_response(stream, 500, "application/json", &json);
                }
            }
        }
        Err(e) => {
            let response = UploadJSResponse {
                success: false,
                message: "JavaScript function validation failed".to_string(),
                function_name: None,
                validation_results: None,
                error: Some(e),
            };
            let json = serde_json::to_string(&response).unwrap_or_default();
            send_http_response(stream, 400, "application/json", &json);
        }
    }
}

pub fn handle_reload_hooks(
    stream: &mut TcpStream,
    request: &str,
    server_admin_token: Arc<Option<String>>,
) {
    // Check admin authentication first
    if let Some(error_response) = check_admin_authentication(request, &server_admin_token) {
        send_http_response(stream, 401, "application/json", &error_response);
        return;
    }

    let hooks_dir = std::env::var("SKILLET_HOOKS_DIR").unwrap_or_else(|_| "hooks".to_string());
    let js_loader = JSPluginLoader::new(hooks_dir);

    match js_loader.auto_register() {
        Ok(count) => {
            let response = ReloadHooksResponse {
                success: true,
                message: format!("Successfully reloaded {} JavaScript function(s)", count),
                functions_loaded: count,
                error: None,
            };
            let json = serde_json::to_string(&response).unwrap_or_default();
            send_http_response(stream, 200, "application/json", &json);
        }
        Err(e) => {
            let response = ReloadHooksResponse {
                success: false,
                message: "Failed to reload JavaScript functions".to_string(),
                functions_loaded: 0,
                error: Some(e.to_string()),
            };
            let json = serde_json::to_string(&response).unwrap_or_default();
            send_http_response(stream, 500, "application/json", &json);
        }
    }
}

fn validate_js_function(js_code: &str) -> Result<(JavaScriptFunction, ValidationResults), String> {
    let mut validation_results = ValidationResults {
        syntax_valid: false,
        structure_valid: false,
        example_test_passed: false,
        example_result: None,
        example_error: None,
    };

    // Step 1: Parse the JS function (validates syntax and structure)
    let js_func = match JavaScriptFunction::parse_js_function(js_code) {
        Ok(func) => {
            validation_results.syntax_valid = true;
            validation_results.structure_valid = true;
            func
        }
        Err(e) => {
            return Err(format!("Syntax/structure validation failed: {}", e));
        }
    };

    // Step 2: Test the example if provided
    if let Some(example) = js_func.example() {
        match test_js_function_example(&js_func, example) {
            Ok(result) => {
                validation_results.example_test_passed = true;
                validation_results.example_result = Some(result);
            }
            Err(e) => {
                validation_results.example_test_passed = false;
                validation_results.example_error = Some(e);
            }
        }
    } else {
        // No example provided, consider it passed
        validation_results.example_test_passed = true;
        validation_results.example_result = Some("No example provided to test".to_string());
    }

    Ok((js_func, validation_results))
}

fn test_js_function_example(js_func: &JavaScriptFunction, example: &str) -> Result<String, String> {
    // Parse the example to extract function call and expected result
    // Example format: "MYFUNCTION(5) returns 10"
    // or "MYFUNCTION(\"hello\") returns \"HELLO\""

    if let Some((call_part, expected_part)) = example.split_once(" returns ") {
        let function_call = call_part.trim();
        let expected_result = expected_part.trim();

        // Parse the function call to get arguments
        if let Some(args_str) = function_call.strip_prefix(&format!("{}(", js_func.name())).and_then(|s| s.strip_suffix(')')) {
            // Simple argument parsing - this could be enhanced
            let args = parse_function_arguments(args_str)?;

            // Execute the function
            match js_func.execute(args) {
                Ok(result) => {
                    let result_str = format_value_for_comparison(&result);
                    if result_str == expected_result || format!("\"{}\"", result_str) == expected_result {
                        Ok(format!("Expected: {}, Got: {} ✓", expected_result, result_str))
                    } else {
                        Err(format!("Expected: {}, Got: {}", expected_result, result_str))
                    }
                }
                Err(e) => {
                    Err(format!("Function execution failed: {}", e))
                }
            }
        } else {
            Err("Invalid example format: cannot parse function call".to_string())
        }
    } else {
        Err("Invalid example format: expected 'FUNCTION(args) returns result'".to_string())
    }
}

fn parse_function_arguments(args_str: &str) -> Result<Vec<Value>, String> {
    if args_str.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut args = Vec::new();

    // Simple argument parsing - handles basic cases
    for arg in args_str.split(',') {
        let arg = arg.trim();

        if arg.starts_with('"') && arg.ends_with('"') {
            // String argument
            let string_val = &arg[1..arg.len()-1]; // Remove quotes
            args.push(Value::String(string_val.to_string()));
        } else if arg == "true" {
            args.push(Value::Boolean(true));
        } else if arg == "false" {
            args.push(Value::Boolean(false));
        } else if let Ok(num) = arg.parse::<f64>() {
            // Number argument
            args.push(Value::Number(num));
        } else {
            return Err(format!("Cannot parse argument: {}", arg));
        }
    }

    Ok(args)
}

fn format_value_for_comparison(value: &Value) -> String {
    match value {
        Value::Number(n) => {
            if n.fract() == 0.0 {
                format!("{}", *n as i64)
            } else {
                format!("{}", n)
            }
        }
        Value::String(s) => s.clone(),
        Value::Boolean(b) => b.to_string(),
        Value::Null => "null".to_string(),
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(format_value_for_comparison).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Currency(c) => format!("{}", c),
        Value::DateTime(dt) => dt.to_string(),
        Value::Json(json) => json.clone(),
    }
}

fn save_js_file(hooks_dir: &str, filename: &str, js_code: &str) -> Result<(), String> {
    use std::path::Path;

    // Ensure hooks directory exists
    let hooks_path = Path::new(hooks_dir);
    if !hooks_path.exists() {
        fs::create_dir_all(hooks_path)
            .map_err(|e| format!("Failed to create hooks directory: {}", e))?;
    }

    // Save the file
    let file_path = hooks_path.join(filename);
    fs::write(&file_path, js_code)
        .map_err(|e| format!("Failed to write JS file: {}", e))?;

    Ok(())
}

fn delete_js_file(hooks_dir: &str, filename: &str) -> Result<(), String> {
    use std::path::Path;

    let hooks_path = Path::new(hooks_dir);
    let file_path = hooks_path.join(filename);

    // Check if file exists
    if !file_path.exists() {
        return Err(format!("File '{}' does not exist", filename));
    }

    // Delete the file
    fs::remove_file(&file_path)
        .map_err(|e| format!("Failed to delete JS file: {}", e))?;

    Ok(())
}

fn list_js_functions(hooks_dir: &str) -> Result<Vec<JSFunctionInfo>, String> {
    use std::path::Path;

    let hooks_path = Path::new(hooks_dir);

    if !hooks_path.exists() {
        // Return empty list if hooks directory doesn't exist
        return Ok(Vec::new());
    }

    let mut functions = Vec::new();

    // Recursively scan directory for JS files
    scan_directory_for_js(hooks_path, hooks_path, &mut functions)?;

    // Sort by filename for consistent ordering
    functions.sort_by(|a, b| a.filename.cmp(&b.filename));

    Ok(functions)
}

fn scan_directory_for_js(
    current_dir: &std::path::Path,
    hooks_root: &std::path::Path,
    functions: &mut Vec<JSFunctionInfo>
) -> Result<(), String> {

    let entries = fs::read_dir(current_dir)
        .map_err(|e| format!("Failed to read directory: {}", e))?;

    for entry in entries {
        let entry = entry
            .map_err(|e| format!("Failed to read directory entry: {}", e))?;

        let path = entry.path();

        if path.is_dir() {
            // Recursively scan subdirectories
            scan_directory_for_js(&path, hooks_root, functions)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("js") {
            // Get relative path from hooks root
            let relative_path = path.strip_prefix(hooks_root)
                .map_err(|_| "Failed to get relative path".to_string())?;

            let filename = relative_path.to_string_lossy().to_string();

            // Get file metadata
            let metadata = entry.metadata()
                .map_err(|e| format!("Failed to get file metadata: {}", e))?;

            let file_size = metadata.len();
            let last_modified = metadata.modified()
                .map(|time| {
                    use std::time::UNIX_EPOCH;
                    let duration = time.duration_since(UNIX_EPOCH).unwrap_or_default();
                    chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                        .unwrap_or_else(|| "Unknown".to_string())
                })
                .unwrap_or_else(|_| "Unknown".to_string());

            // Try to parse the JS function to get metadata
            let (function_name, description, example, min_args, max_args, is_valid, validation_error) =
                match fs::read_to_string(&path) {
                    Ok(content) => {
                        match JavaScriptFunction::parse_js_function(&content) {
                            Ok(js_func) => (
                                Some(js_func.name().to_string()),
                                js_func.description().map(|s| s.to_string()),
                                js_func.example().map(|s| s.to_string()),
                                Some(js_func.min_args()),
                                js_func.max_args(),
                                true,
                                None,
                            ),
                            Err(e) => (None, None, None, None, None, false, Some(e.to_string())),
                        }
                    }
                    Err(e) => (None, None, None, None, None, false, Some(format!("Failed to read file: {}", e))),
                };

            functions.push(JSFunctionInfo {
                filename,
                function_name,
                description,
                example,
                min_args,
                max_args,
                file_size,
                last_modified,
                is_valid,
                validation_error,
            });
        }
    }

    Ok(())
}

fn parse_upload_request(request: &str) -> Result<UploadJSRequest, String> {
    // Extract Content-Type header
    let content_type = extract_content_type(request).unwrap_or_default();
    
    if is_multipart_request(&content_type) {
        // Parse multipart form data
        let boundary = extract_boundary_from_content_type(&content_type)
            .ok_or("Missing boundary in multipart Content-Type")?;
        
        let body = extract_request_body(request)?;
        let multipart_data = parse_multipart_data(&body, &boundary)?;
        
        // Extract filename from form field
        let filename = multipart_data.get_text_field("filename")
            .ok_or("Missing 'filename' field in multipart data")?;
        
        // Check for JS code in text field or file field
        let (js_code, file_content) = if let Some(js_text) = multipart_data.get_text_field("js_code") {
            // JS code provided as text field
            (Some(js_text), None)
        } else if let Some(file_field) = multipart_data.get_file_field("file") {
            // JS code provided as file upload
            let file_content = String::from_utf8(file_field.content.clone())
                .map_err(|_| "Uploaded file contains invalid UTF-8")?;
            (None, Some(file_content))
        } else {
            return Err("Either 'js_code' text field or 'file' upload field must be provided".to_string());
        };
        
        Ok(UploadJSRequest {
            filename,
            js_code,
            file_content,
        })
    } else {
        // Parse as JSON
        parse_json_body(request)
    }
}

fn parse_update_request(request: &str) -> Result<UpdateJSRequest, String> {
    // Extract Content-Type header
    let content_type = extract_content_type(request).unwrap_or_default();
    
    if is_multipart_request(&content_type) {
        // Parse multipart form data
        let boundary = extract_boundary_from_content_type(&content_type)
            .ok_or("Missing boundary in multipart Content-Type")?;
        
        let body = extract_request_body(request)?;
        let multipart_data = parse_multipart_data(&body, &boundary)?;
        
        // Extract filename from form field
        let filename = multipart_data.get_text_field("filename")
            .ok_or("Missing 'filename' field in multipart data")?;
        
        // Check for JS code in text field or file field
        let (js_code, file_content) = if let Some(js_text) = multipart_data.get_text_field("js_code") {
            // JS code provided as text field
            (Some(js_text), None)
        } else if let Some(file_field) = multipart_data.get_file_field("file") {
            // JS code provided as file upload
            let file_content = String::from_utf8(file_field.content.clone())
                .map_err(|_| "Uploaded file contains invalid UTF-8")?;
            (None, Some(file_content))
        } else {
            return Err("Either 'js_code' text field or 'file' upload field must be provided".to_string());
        };
        
        Ok(UpdateJSRequest {
            filename,
            js_code,
            file_content,
        })
    } else {
        // Parse as JSON
        parse_json_body(request)
    }
}

fn extract_content_type(request: &str) -> Option<String> {
    for line in request.lines() {
        if line.to_lowercase().starts_with("content-type:") {
            return Some(line.split(':').nth(1)?.trim().to_string());
        }
    }
    None
}

fn extract_request_body(request: &str) -> Result<String, String> {
    if let Some(body_start) = request.find("\r\n\r\n") {
        Ok(request[body_start + 4..].to_string())
    } else if let Some(body_start) = request.find("\n\n") {
        Ok(request[body_start + 2..].to_string())
    } else {
        Err("Could not find request body separator".to_string())
    }
}