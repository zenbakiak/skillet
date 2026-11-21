//! Cross-origin policy of `sk_http_server` (ADR-0019).
//!
//! `Access-Control-Allow-Origin` is the configured origin (`--cors-origin` /
//! `SKILLET_CORS_ORIGIN`), else `*` when a token is configured, else absent: a server in
//! development mode is not readable from pages on other origins. Requests with a body must
//! be `application/json`, so a cross-origin browser request always needs a preflight.

use std::sync::OnceLock;

/// The CORS header lines sent with every response; empty when no origin is allowed.
static CORS_HEADERS: OnceLock<String> = OnceLock::new();

/// The `Access-Control-Allow-Origin` value: the configured one, else `*` when a token is
/// configured, else none.
pub fn allowed_origin(configured: Option<&str>, token_configured: bool) -> Option<String> {
    match configured {
        Some(origin) => Some(origin.to_string()),
        None if token_configured => Some("*".to_string()),
        None => None,
    }
}

/// Set the policy once at start-up. Before this runs no CORS headers are sent.
pub fn init(origin: Option<&str>) {
    let headers = match origin {
        Some(origin) => format!(
            "Access-Control-Allow-Origin: {origin}\r\n\
             Access-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS\r\n\
             Access-Control-Allow-Headers: Authorization, Content-Type\r\n"
        ),
        None => String::new(),
    };
    let _ = CORS_HEADERS.set(headers);
}

/// Header lines (each ending in `\r\n`) to add to a response.
pub fn response_headers() -> &'static str {
    CORS_HEADERS.get().map(String::as_str).unwrap_or("")
}

/// An origin value is usable in a header: non-empty, no whitespace or control characters.
pub fn valid_origin(origin: &str) -> bool {
    !origin.is_empty() && !origin.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// Value of header `name` (case-insensitive), looking only at the header section.
fn header<'a>(request: &'a str, name: &str) -> Option<&'a str> {
    let head = request.split("\r\n\r\n").next().unwrap_or(request);
    head.lines().skip(1).find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim()
            .eq_ignore_ascii_case(name)
            .then_some(value.trim())
    })
}

/// The media type of `Content-Type` without parameters, lower-cased.
fn media_type(request: &str) -> Option<String> {
    let value = header(request, "content-type")?;
    let essence = value.split(';').next().unwrap_or("").trim();
    Some(essence.to_ascii_lowercase())
}

fn has_body(request: &str) -> bool {
    header(request, "content-length")
        .and_then(|v| v.parse::<usize>().ok())
        .is_some_and(|n| n > 0)
}

/// Paths that need the admin token.
const ADMIN_PATHS: [&str; 6] = [
    "/upload-js",
    "/update-js",
    "/delete-js",
    "/list-js",
    "/reload-hooks",
    "/cache",
];

/// `Err(message)` for a 403: in development mode (no admin token) a browser request (an
/// `Origin` header) to an admin endpoint, whatever its method and body. Browsers send some
/// requests without a preflight (multipart uploads, body-less POSTs), so CORS alone does not
/// stop a page from driving these endpoints.
pub fn check_browser_admin(
    path: &str,
    request: &str,
    admin_token_configured: bool,
) -> Result<(), &'static str> {
    if admin_token_configured || !ADMIN_PATHS.contains(&path) || header(request, "origin").is_none()
    {
        return Ok(());
    }
    Err(
        "Admin endpoints refuse browser requests (Origin header) in development mode; \
         set an admin token (--admin-token or SKILLET_ADMIN_TOKEN)",
    )
}

/// `Err(message)` for a 415: a request with a body that is not `application/json`.
/// `/upload-js` and `/update-js` also accept `multipart/form-data`.
pub fn check_content_type(method: &str, path: &str, request: &str) -> Result<(), &'static str> {
    if matches!(method, "GET" | "HEAD" | "OPTIONS") || !has_body(request) {
        return Ok(());
    }
    match media_type(request).as_deref() {
        Some("application/json") => Ok(()),
        Some("multipart/form-data") if matches!(path, "/upload-js" | "/update-js") => Ok(()),
        _ => Err("Content-Type must be application/json"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(headers: &str) -> String {
        format!("POST /eval HTTP/1.1\r\nHost: x\r\n{headers}Content-Length: 2\r\n\r\n{{}}")
    }

    #[test]
    fn allowed_origin_follows_the_three_cases() {
        assert_eq!(
            allowed_origin(Some("https://a.example"), false).as_deref(),
            Some("https://a.example")
        );
        assert_eq!(allowed_origin(Some("*"), false).as_deref(), Some("*"));
        assert_eq!(allowed_origin(None, true).as_deref(), Some("*"));
        assert_eq!(allowed_origin(None, false), None);
    }

    #[test]
    fn json_with_parameters_is_accepted_case_insensitively() {
        for ct in [
            "application/json",
            "Application/JSON; charset=utf-8",
            "application/json;charset=UTF-8",
        ] {
            let r = req(&format!("content-type: {ct}\r\n"));
            assert_eq!(check_content_type("POST", "/eval", &r), Ok(()), "{ct}");
        }
    }

    #[test]
    fn other_or_missing_content_types_are_rejected() {
        for headers in [
            "",
            "Content-Type: text/plain\r\n",
            "Content-Type: application/x-www-form-urlencoded\r\n",
            "Content-Type: application/jsonx\r\n",
        ] {
            assert!(
                check_content_type("POST", "/eval", &req(headers)).is_err(),
                "{headers:?}"
            );
            assert!(
                check_content_type("DELETE", "/delete-js", &req(headers)).is_err(),
                "{headers:?}"
            );
        }
    }

    #[test]
    fn requests_without_a_body_are_not_checked() {
        let r = "DELETE /cache HTTP/1.1\r\nHost: x\r\n\r\n";
        assert_eq!(check_content_type("DELETE", "/cache", r), Ok(()));
        let r = "POST /reload-hooks HTTP/1.1\r\nContent-Length: 0\r\n\r\n";
        assert_eq!(check_content_type("POST", "/reload-hooks", r), Ok(()));
    }

    #[test]
    fn multipart_only_on_upload_endpoints() {
        let mp = "Content-Type: multipart/form-data; boundary=x\r\n";
        assert!(check_content_type("POST", "/eval", &req(mp)).is_err());
        assert_eq!(check_content_type("POST", "/upload-js", &req(mp)), Ok(()));
        assert_eq!(check_content_type("PUT", "/update-js", &req(mp)), Ok(()));
    }

    #[test]
    fn browser_requests_to_admin_endpoints_need_an_admin_token() {
        let browser = "GET /list-js HTTP/1.1\r\nOrigin: https://evil.example\r\n\r\n";
        let plain = "GET /list-js HTTP/1.1\r\nHost: x\r\n\r\n";
        for path in ADMIN_PATHS {
            assert!(check_browser_admin(path, browser, false).is_err(), "{path}");
            assert_eq!(check_browser_admin(path, browser, true), Ok(()), "{path}");
            assert_eq!(check_browser_admin(path, plain, false), Ok(()), "{path}");
        }
        assert_eq!(check_browser_admin("/eval", browser, false), Ok(()));
        // An `Origin` line in the body is not a header.
        let body = "POST /reload-hooks HTTP/1.1\r\nContent-Length: 20\r\n\r\nOrigin: https://x.y";
        assert_eq!(check_browser_admin("/reload-hooks", body, false), Ok(()));
    }

    #[test]
    fn body_text_is_not_read_as_headers() {
        let r = "POST /eval HTTP/1.1\r\nContent-Length: 30\r\n\r\nContent-Type: application/json";
        assert!(check_content_type("POST", "/eval", r).is_err());
    }

    #[test]
    fn origins_with_whitespace_or_control_characters_are_invalid() {
        assert!(valid_origin("https://app.example:8443"));
        assert!(valid_origin("*"));
        assert!(!valid_origin(""));
        assert!(!valid_origin("https://a\r\nX-Evil: 1"));
        assert!(!valid_origin("a b"));
    }
}
