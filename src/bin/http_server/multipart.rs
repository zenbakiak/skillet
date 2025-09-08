use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct MultipartField {
    pub name: String,
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub content: Vec<u8>,
}

#[derive(Debug)]
pub struct MultipartData {
    pub fields: HashMap<String, MultipartField>,
}

impl MultipartData {
    pub fn get_field(&self, name: &str) -> Option<&MultipartField> {
        self.fields.get(name)
    }

    pub fn get_text_field(&self, name: &str) -> Option<String> {
        self.fields.get(name)
            .and_then(|field| String::from_utf8(field.content.clone()).ok())
    }

    pub fn get_file_field(&self, name: &str) -> Option<&MultipartField> {
        self.fields.get(name)
            .filter(|field| field.filename.is_some())
    }
}

pub fn parse_multipart_data(body: &str, boundary: &str) -> Result<MultipartData, String> {
    let boundary_marker = format!("--{}", boundary);
    let end_boundary = format!("--{}--", boundary);
    
    let mut fields = HashMap::new();
    let parts: Vec<&str> = body.split(&boundary_marker).collect();
    
    for part in parts.iter().skip(1) { // Skip the first empty part
        let part = part.trim();
        
        // Skip the end boundary
        if part.starts_with(&end_boundary[2..]) || part.is_empty() {
            continue;
        }
        
        // Find the separation between headers and content
        if let Some((headers_section, content_section)) = part.split_once("\r\n\r\n") {
            let field = parse_multipart_field(headers_section, content_section)?;
            fields.insert(field.name.clone(), field);
        } else if let Some((headers_section, content_section)) = part.split_once("\n\n") {
            // Handle cases with just \n instead of \r\n
            let field = parse_multipart_field(headers_section, content_section)?;
            fields.insert(field.name.clone(), field);
        }
    }
    
    Ok(MultipartData { fields })
}

fn parse_multipart_field(headers_section: &str, content_section: &str) -> Result<MultipartField, String> {
    let mut name = String::new();
    let mut filename = None;
    let mut content_type = None;
    
    // Parse headers
    for line in headers_section.lines() {
        let line = line.trim();
        
        if line.to_lowercase().starts_with("content-disposition:") {
            // Parse Content-Disposition header
            // Example: Content-Disposition: form-data; name="file"; filename="test.js"
            let params = parse_content_disposition(line)?;
            
            if let Some(field_name) = params.get("name") {
                name = field_name.clone();
            }
            
            filename = params.get("filename").cloned();
            
        } else if line.to_lowercase().starts_with("content-type:") {
            // Parse Content-Type header
            content_type = Some(line.split(':').nth(1)
                .unwrap_or("application/octet-stream")
                .trim()
                .to_string());
        }
    }
    
    if name.is_empty() {
        return Err("Missing field name in multipart data".to_string());
    }
    
    // Clean up content - remove trailing boundary markers and whitespace
    let content = content_section
        .trim_end_matches("--")
        .trim()
        .as_bytes()
        .to_vec();
    
    Ok(MultipartField {
        name,
        filename,
        content_type,
        content,
    })
}

fn parse_content_disposition(header: &str) -> Result<HashMap<String, String>, String> {
    let mut params = HashMap::new();
    
    // Skip "Content-Disposition: " prefix
    let value_part = header.split(':').nth(1)
        .ok_or("Invalid Content-Disposition header")?
        .trim();
    
    // Split by semicolon and parse each parameter
    for param in value_part.split(';') {
        let param = param.trim();
        
        if param.contains('=') {
            let mut parts = param.splitn(2, '=');
            let key = parts.next().unwrap().trim().to_string();
            let value = parts.next().unwrap().trim();
            
            // Remove quotes from value if present
            let cleaned_value = if value.starts_with('"') && value.ends_with('"') {
                value[1..value.len()-1].to_string()
            } else {
                value.to_string()
            };
            
            params.insert(key, cleaned_value);
        }
    }
    
    Ok(params)
}

pub fn extract_boundary_from_content_type(content_type: &str) -> Option<String> {
    // Parse boundary from Content-Type header
    // Example: multipart/form-data; boundary=----WebKitFormBoundary7MA4YWxkTrZu0gW
    
    for part in content_type.split(';') {
        let part = part.trim();
        if part.starts_with("boundary=") {
            return Some(part[9..].to_string()); // Remove "boundary=" prefix
        }
    }
    
    None
}

/// Determine if the request is multipart form-data
pub fn is_multipart_request(content_type: &str) -> bool {
    content_type.to_lowercase().starts_with("multipart/form-data")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boundary_extraction() {
        let content_type = "multipart/form-data; boundary=----WebKitFormBoundary7MA4YWxkTrZu0gW";
        let boundary = extract_boundary_from_content_type(content_type).unwrap();
        assert_eq!(boundary, "----WebKitFormBoundary7MA4YWxkTrZu0gW");
    }

    #[test]
    fn test_content_disposition_parsing() {
        let header = "Content-Disposition: form-data; name=\"file\"; filename=\"test.js\"";
        let params = parse_content_disposition(header).unwrap();
        
        assert_eq!(params.get("name").unwrap(), "file");
        assert_eq!(params.get("filename").unwrap(), "test.js");
    }

    #[test]
    fn test_multipart_parsing() {
        let body = r#"------WebKitFormBoundary7MA4YWxkTrZu0gW
Content-Disposition: form-data; name="filename"

test.js
------WebKitFormBoundary7MA4YWxkTrZu0gW
Content-Disposition: form-data; name="file"; filename="test.js"
Content-Type: application/javascript

function test() { return 42; }
------WebKitFormBoundary7MA4YWxkTrZu0gW--"#;

        let multipart = parse_multipart_data(body, "----WebKitFormBoundary7MA4YWxkTrZu0gW").unwrap();
        
        assert_eq!(multipart.get_text_field("filename").unwrap(), "test.js");
        
        let file_field = multipart.get_file_field("file").unwrap();
        assert_eq!(file_field.filename.as_ref().unwrap(), "test.js");
        assert_eq!(String::from_utf8(file_field.content.clone()).unwrap().trim(), "function test() { return 42; }");
    }
}