use serde::de::DeserializeOwned;
use serde_json;
use std::io::{Read, Write};
use std::net::TcpStream;

use super::cache::{get_pooled_buffer, return_pooled_buffer};
use super::request_limits::{DeadlineReader, MAX_REQUEST_SIZE, WRITE_TIMEOUT};

pub fn sanitize_json_key(key: &str) -> String {
    // Fast path: if key is already valid, return as-is (no allocation)
    if key.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return key.to_string();
    }

    // Slow path: sanitize invalid characters
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

/// Read one HTTP request of at most `MAX_REQUEST_SIZE` bytes. The whole request must arrive
/// within `REQUEST_READ_DEADLINE` (BL-037); a slow client cannot extend it read by read.
pub fn read_complete_http_request(stream: &mut TcpStream) -> Result<String, std::io::Error> {
    stream.set_write_timeout(Some(WRITE_TIMEOUT))?;
    let mut reader = DeadlineReader::new(stream);

    let mut buffer = get_pooled_buffer();
    let mut temp_buffer = [0; 4096]; // Increased buffer size for better performance
    let mut headers_complete = false;
    let mut content_length: usize = 0;
    let mut headers_end_pos = 0;

    // First, read until we have complete headers
    while !headers_complete {
        let bytes_read = reader.read(&mut temp_buffer)?;
        if bytes_read == 0 {
            break;
        }

        buffer.extend_from_slice(&temp_buffer[..bytes_read]);

        // Check request size limit
        if buffer.len() > MAX_REQUEST_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Request too large (max 1MB)",
            ));
        }

        // Check if we have complete headers (ending with \r\n\r\n)
        if let Some(pos) = find_headers_end(&buffer) {
            headers_complete = true;
            headers_end_pos = pos + 4;

            // Parse the headers to get Content-Length
            let headers_str = String::from_utf8_lossy(&buffer[..pos]);
            content_length = parse_content_length(&headers_str);

            // Validate content length
            if content_length > MAX_REQUEST_SIZE {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Request body too large (max 1MB)",
                ));
            }
        }
    }

    // Now read the remaining body if needed
    let body_bytes_read = buffer.len() - headers_end_pos;
    let remaining_bytes = content_length.saturating_sub(body_bytes_read);

    if remaining_bytes > 0 {
        // Reserve space in buffer to avoid multiple reallocations
        buffer.reserve(remaining_bytes);

        let mut temp_body_buffer = [0; 8192]; // Larger read buffer
        let mut total_read = 0;

        while total_read < remaining_bytes {
            let to_read = std::cmp::min(temp_body_buffer.len(), remaining_bytes - total_read);
            let bytes_read = reader.read(&mut temp_body_buffer[..to_read])?;
            if bytes_read == 0 {
                break; // Connection closed by client
            }

            buffer.extend_from_slice(&temp_body_buffer[..bytes_read]);
            total_read += bytes_read;

            // Additional safety check
            if buffer.len() > MAX_REQUEST_SIZE {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Request exceeded size limit during body read",
                ));
            }
        }
    }

    // Convert buffer to string without cloning (consume the buffer)
    String::from_utf8(buffer).map_err(|e| {
        // If conversion fails, we can't return the buffer to pool (it was consumed)
        // but we return the original buffer via the error
        let original_buffer = e.into_bytes();
        return_pooled_buffer(original_buffer);
        std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid UTF-8")
    })
}

fn find_headers_end(buffer: &[u8]) -> Option<usize> {
    let pattern = b"\r\n\r\n";
    buffer
        .windows(pattern.len())
        .position(|window| window == pattern)
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
        408 => "Request Timeout",
        413 => "Payload Too Large",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Unknown",
    };

    // Headers and body are written separately, so a large body is not copied again.
    let headers = format!(
        "HTTP/1.1 {} {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Access-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS\r\n\
         Access-Control-Allow-Headers: Content-Type, Authorization\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n",
        status,
        status_text,
        content_type,
        body.len(),
    );

    let _ = stream
        .write_all(headers.as_bytes())
        .and_then(|()| stream.write_all(body.as_bytes()));
}

/// Serialize `value` into a string allocated once at its exact size: a first pass counts the
/// bytes, so a large response never holds two growing copies (ADR-0011 amendment 2).
pub fn to_json_exact<T: serde::Serialize>(value: &T) -> String {
    struct Count(usize);
    impl std::io::Write for Count {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0 += buf.len();
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut count = Count(0);
    if serde_json::to_writer(&mut count, value).is_err() {
        return String::new();
    }
    let mut out = Vec::with_capacity(count.0);
    match serde_json::to_writer(&mut out, value) {
        // serde_json only writes valid UTF-8.
        Ok(()) => String::from_utf8(out).unwrap_or_default(),
        Err(_) => String::new(),
    }
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

/// The `GET /` page, embedded at build time so an installed binary serves it from any
/// working directory (BL-045).
pub fn load_html_file() -> &'static str {
    include_str!("documentation.html")
}
