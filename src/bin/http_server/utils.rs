use std::net::TcpStream;
use std::io::{Read, Write};
use serde::de::DeserializeOwned;
use serde_json;

pub fn sanitize_json_key(key: &str) -> String {
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

pub fn read_complete_http_request(stream: &mut TcpStream) -> Result<String, std::io::Error> {
    let mut buffer = Vec::new();
    let mut temp_buffer = [0; 1024];
    let mut headers_complete = false;
    let mut content_length: usize = 0;
    let mut headers_end_pos = 0;

    // First, read until we have complete headers
    while !headers_complete {
        let bytes_read = stream.read(&mut temp_buffer)?;
        if bytes_read == 0 {
            break;
        }

        buffer.extend_from_slice(&temp_buffer[..bytes_read]);

        // Check if we have complete headers (ending with \r\n\r\n)
        if let Some(pos) = find_headers_end(&buffer) {
            headers_complete = true;
            headers_end_pos = pos + 4;

            // Parse the headers to get Content-Length
            let headers_str = String::from_utf8_lossy(&buffer[..pos]);
            content_length = parse_content_length(&headers_str);
        }
    }

    // Now read the remaining body if needed
    let body_bytes_read = buffer.len() - headers_end_pos;
    let remaining_bytes = content_length.saturating_sub(body_bytes_read);

    if remaining_bytes > 0 {
        let mut body_buffer = vec![0; remaining_bytes];
        let mut total_read = 0;

        while total_read < remaining_bytes {
            let bytes_read = stream.read(&mut body_buffer[total_read..])?;
            if bytes_read == 0 {
                break;
            }
            total_read += bytes_read;
        }

        buffer.extend_from_slice(&body_buffer[..total_read]);
    }

    String::from_utf8(buffer).map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid UTF-8"))
}

fn find_headers_end(buffer: &[u8]) -> Option<usize> {
    let pattern = b"\r\n\r\n";
    buffer.windows(pattern.len()).position(|window| window == pattern)
}

fn parse_content_length(headers: &str) -> usize {
    for line in headers.lines() {
        if line.to_lowercase().starts_with("content-length:") {
            if let Some(value) = line.split(':').nth(1) {
                return value.trim().parse().unwrap_or(0);
            }
        }
    }
    0
}

pub fn send_http_response(stream: &mut TcpStream, status: u16, content_type: &str, body: &str) {
    let status_text = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized", 
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Unknown",
    };

    let response = format!(
        "HTTP/1.1 {} {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Access-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS\r\n\
         Access-Control-Allow-Headers: Content-Type, Authorization\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        status, status_text, content_type, body.len(), body
    );

    let _ = stream.write_all(response.as_bytes());
}

pub fn send_http_error(stream: &mut TcpStream, status: u16, message: &str) {
    let error_json = serde_json::json!({
        "success": false,
        "error": message
    });
    send_http_response(stream, status, "application/json", &error_json.to_string());
}

pub fn handle_cors_preflight(stream: &mut TcpStream) {
    let response = "HTTP/1.1 200 OK\r\n\
        Access-Control-Allow-Origin: *\r\n\
        Access-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS\r\n\
        Access-Control-Allow-Headers: Content-Type, Authorization\r\n\
        Content-Length: 0\r\n\
        \r\n";
    let _ = stream.write_all(response.as_bytes());
}

pub fn parse_json_body<T: DeserializeOwned>(request: &str) -> Result<T, String> {
    // Find the JSON body after headers
    let body_start = match request.find("\r\n\r\n") {
        Some(pos) => pos + 4,
        None => return Err("Invalid HTTP request".to_string()),
    };

    let body = &request[body_start..];
    serde_json::from_str(body).map_err(|e| format!("Invalid JSON: {}", e))
}

pub fn load_html_file() -> String {
    match std::fs::read_to_string("src/bin/http_server/documentation.html") {
        Ok(content) => content,
        Err(_) => {
            // Fallback HTML if file not found
            r#"<!DOCTYPE html>
<html>
<head><title>Skillet Expression Server</title></head>
<body>
    <h1>🥘 Skillet Expression Server</h1>
    <p>Documentation file not found. Please ensure documentation.html is available.</p>
</body>
</html>"#.to_string()
        }
    }
}