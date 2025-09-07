use crate::custom::CustomFunction;
use crate::error::Error;
use crate::types::Value;
use rquickjs::{Runtime, Function as JsFunction, FromJs, IntoJs, Ctx};
use std::fs;
use std::path::Path;

/// A custom function implemented in JavaScript
pub struct JavaScriptFunction {
    name: String,
    min_args: usize,
    max_args: Option<usize>,
    description: Option<String>,
    example: Option<String>,
    js_code: String,
}

impl JavaScriptFunction {
    /// Create a new JavaScript function from source code
    pub fn new(
        name: String,
        min_args: usize,
        max_args: Option<usize>,
        description: Option<String>,
        example: Option<String>,
        js_code: String,
    ) -> Result<Self, Error> {
        Ok(Self {
            name,
            min_args,
            max_args,
            description,
            example,
            js_code,
        })
    }

    /// Parse JavaScript function definition from source code (public method)
    pub fn parse_js_function(js_code: &str) -> Result<Self, Error> {
        Self::parse_js_function_internal(js_code)
    }

    /// Load a JavaScript function from a file
    pub fn from_file<P: AsRef<Path>>(file_path: P) -> Result<Self, Error> {
        let content = fs::read_to_string(&file_path)
            .map_err(|e| Error::new(format!("Failed to read JS file: {}", e), None))?;
        
        Self::parse_js_function_internal(&content)
    }

    /// Parse JavaScript function definition from source code
    /// Expected format:
    /// ```javascript
    /// // @name: MYFUNCTION
    /// // @min_args: 1
    /// // @max_args: 2
    /// // @description: My custom function
    /// // @example: MYFUNCTION(5) returns 10
    /// function execute(args) {
    ///     // Implementation here
    ///     return args[0] * 2;
    /// }
    /// ```
    fn parse_js_function_internal(js_code: &str) -> Result<Self, Error> {
        let mut name = None;
        let mut min_args = 1;
        let mut max_args = None;
        let mut description = None;
        let mut example = None;

        // Parse metadata from comments
        for line in js_code.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("// @name:") {
                name = Some(rest.trim().to_string());
            } else if let Some(rest) = line.strip_prefix("// @min_args:") {
                min_args = rest.trim().parse()
                    .map_err(|_| Error::new("Invalid min_args value", None))?;
            } else if let Some(rest) = line.strip_prefix("// @max_args:") {
                if rest.trim() == "unlimited" {
                    max_args = None;
                } else {
                    max_args = Some(rest.trim().parse()
                        .map_err(|_| Error::new("Invalid max_args value", None))?);
                }
            } else if let Some(rest) = line.strip_prefix("// @description:") {
                description = Some(rest.trim().to_string());
            } else if let Some(rest) = line.strip_prefix("// @example:") {
                example = Some(rest.trim().to_string());
            }
        }

        let name = name.ok_or_else(|| Error::new("JavaScript function must have @name annotation", None))?;

        Self::new(name, min_args, max_args, description, example, js_code.to_string())
    }

    /// Convert Skillet Value to JavaScript value
    fn value_to_js<'js>(ctx: &Ctx<'js>, value: &Value) -> Result<rquickjs::Value<'js>, Error> {
        match value {
            Value::Number(n) => n.into_js(ctx).map_err(|e| Error::new(format!("JS conversion error: {}", e), None)),
            Value::String(s) => s.clone().into_js(ctx).map_err(|e| Error::new(format!("JS conversion error: {}", e), None)),
            Value::Boolean(b) => b.into_js(ctx).map_err(|e| Error::new(format!("JS conversion error: {}", e), None)),
            Value::Null => ().into_js(ctx).map_err(|e| Error::new(format!("JS conversion error: {}", e), None)),
            Value::Array(arr) => {
                let js_array = rquickjs::Array::new(ctx.clone())
                    .map_err(|e| Error::new(format!("Failed to create JS array: {}", e), None))?;
                
                for (i, item) in arr.iter().enumerate() {
                    let js_val = Self::value_to_js(ctx, item)?;
                    js_array.set(i, js_val)
                        .map_err(|e| Error::new(format!("Failed to set array element: {}", e), None))?;
                }
                
                js_array.into_js(ctx).map_err(|e| Error::new(format!("JS conversion error: {}", e), None))
            }
            Value::Currency(c) => c.into_js(ctx).map_err(|e| Error::new(format!("JS conversion error: {}", e), None)),
            Value::DateTime(dt) => (*dt as f64).into_js(ctx).map_err(|e| Error::new(format!("JS conversion error: {}", e), None)),
            Value::Json(json_str) => {
                // For JSON, we'll just convert to string for now
                json_str.clone().into_js(ctx).map_err(|e| Error::new(format!("JS conversion error: {}", e), None))
            }
        }
    }

    /// Add HTTP functions to the JavaScript context
    fn add_http_functions(ctx: &Ctx) -> Result<(), Error> {
        // Create a synchronous HTTP GET function using curl
        let http_get_fn = rquickjs::Function::new(ctx.clone(), |url: String| {
            match std::process::Command::new("curl")
                .arg("-sS") // silent, but show errors
                .arg(&url)
                .output() {
                Ok(output) => {
                    if output.status.success() {
                        String::from_utf8_lossy(&output.stdout).to_string()
                    } else {
                        format!("HTTP error: {}", String::from_utf8_lossy(&output.stderr))
                    }
                }
                Err(e) => format!("Failed to execute curl: {}", e),
            }
        }).map_err(|e| Error::new(format!("Failed to create HTTP function: {}", e), None))?;
        
        // Add the function to global scope
        ctx.globals().set("httpGet", http_get_fn)
            .map_err(|e| Error::new(format!("Failed to set HTTP function: {}", e), None))?;
            
        Ok(())
    }
    

    /// Convert JavaScript value to Skillet Value
    fn js_to_value<'js>(ctx: &Ctx<'js>, js_val: rquickjs::Value<'js>) -> Result<Value, Error> {
        if js_val.is_null() || js_val.is_undefined() {
            Ok(Value::Null)
        } else if js_val.is_bool() {
            let b: bool = FromJs::from_js(ctx, js_val)
                .map_err(|e| Error::new(format!("JS conversion error: {}", e), None))?;
            Ok(Value::Boolean(b))
        } else if js_val.is_number() {
            let n: f64 = FromJs::from_js(ctx, js_val)
                .map_err(|e| Error::new(format!("JS conversion error: {}", e), None))?;
            Ok(Value::Number(n))
        } else if js_val.is_string() {
            let s: String = FromJs::from_js(ctx, js_val)
                .map_err(|e| Error::new(format!("JS conversion error: {}", e), None))?;
            Ok(Value::String(s))
        } else if js_val.is_array() {
            let js_array: rquickjs::Array = FromJs::from_js(ctx, js_val)
                .map_err(|e| Error::new(format!("JS conversion error: {}", e), None))?;
            
            let mut result = Vec::new();
            let length = js_array.len();
            
            for i in 0..length {
                let item = js_array.get::<rquickjs::Value>(i)
                    .map_err(|e| Error::new(format!("Failed to get array element: {}", e), None))?;
                result.push(Self::js_to_value(ctx, item)?);
            }
            
            Ok(Value::Array(result))
        } else if js_val.is_object() {
            // For objects, use JSON.stringify to convert to proper JSON
            let json_stringify: rquickjs::Function = ctx
                .globals()
                .get("JSON")
                .and_then(|json: rquickjs::Object| json.get("stringify"))
                .map_err(|e| Error::new(format!("Failed to get JSON.stringify: {}", e), None))?;
                
            let json_result: rquickjs::Value = json_stringify.call((js_val,))
                .map_err(|e| Error::new(format!("JSON.stringify failed: {}", e), None))?;
                
            if json_result.is_string() {
                let json_string: String = FromJs::from_js(ctx, json_result)
                    .map_err(|e| Error::new(format!("Failed to convert JSON string: {}", e), None))?;
                Ok(Value::Json(json_string))
            } else {
                // Fallback for non-serializable objects
                Ok(Value::String("[object Object]".to_string()))
            }
        } else {
            // For other types, convert to string representation
            let s: String = FromJs::from_js(ctx, js_val)
                .unwrap_or_else(|_| "[unknown type]".to_string());
            Ok(Value::String(s))
        }
    }
}

impl CustomFunction for JavaScriptFunction {
    fn name(&self) -> &str {
        &self.name
    }

    fn min_args(&self) -> usize {
        self.min_args
    }

    fn max_args(&self) -> Option<usize> {
        self.max_args
    }

    fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
        // Create a new runtime for each execution to avoid threading issues
        let runtime = Runtime::new()
            .map_err(|e| Error::new(format!("Failed to create JS runtime: {}", e), None))?;

        let ctx = rquickjs::Context::full(&runtime)
            .map_err(|e| Error::new(format!("Failed to create JS context: {}", e), None))?;

        ctx.with(|ctx| {
            // Add HTTP functionality to the JavaScript context
            Self::add_http_functions(&ctx)?;
            
            // SQLite integration removed
            
            // Execute the JavaScript code
            ctx.eval::<(), _>(self.js_code.as_bytes())
                .map_err(|e| Error::new(format!("JS execution error: {}", e), None))?;

            // Get the execute function
            let execute_fn: JsFunction = ctx.globals().get("execute")
                .map_err(|e| Error::new(format!("Function 'execute' not found in JS code: {}", e), None))?;

            // Convert Skillet values to JavaScript values
            let js_args = rquickjs::Array::new(ctx.clone())
                .map_err(|e| Error::new(format!("Failed to create JS array: {}", e), None))?;

            for (i, arg) in args.iter().enumerate() {
                let js_val = Self::value_to_js(&ctx, arg)?;
                js_args.set(i, js_val)
                    .map_err(|e| Error::new(format!("Failed to set argument: {}", e), None))?;
            }

            // Call the JavaScript function
            let result: rquickjs::Value = execute_fn.call((js_args,))
                .map_err(|e| Error::new(format!("JS function execution failed: {}", e), None))?;

            // Convert result back to Skillet Value
            Self::js_to_value(&ctx, result)
        })
    }

    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    fn example(&self) -> Option<&str> {
        self.example.as_deref()
    }
}

/// JavaScript plugin loader
pub struct JSPluginLoader {
    hooks_dir: String,
}

impl JSPluginLoader {
    /// Create a new plugin loader for the specified hooks directory
    pub fn new(hooks_dir: String) -> Self {
        Self { hooks_dir }
    }

    /// Load all JavaScript functions from the hooks directory (recursively)
    pub fn load_functions(&self) -> Result<Vec<Box<dyn CustomFunction>>, Error> {
        let hooks_path = Path::new(&self.hooks_dir);
        
        if !hooks_path.exists() {
            // Create hooks directory if it doesn't exist
            fs::create_dir_all(hooks_path)
                .map_err(|e| Error::new(format!("Failed to create hooks directory: {}", e), None))?;
            return Ok(Vec::new());
        }

        let mut functions = Vec::new();
        self.load_functions_recursive(hooks_path, &mut functions)?;
        Ok(functions)
    }
    
    /// Recursively load JavaScript functions from a directory
    fn load_functions_recursive(&self, dir: &Path, functions: &mut Vec<Box<dyn CustomFunction>>) -> Result<(), Error> {
        let entries = fs::read_dir(dir)
            .map_err(|e| Error::new(format!("Failed to read directory: {}", e), None))?;

        for entry in entries {
            let entry = entry
                .map_err(|e| Error::new(format!("Failed to read directory entry: {}", e), None))?;
            
            let path = entry.path();
            
            if path.is_dir() {
                // Recursively search subdirectories
                self.load_functions_recursive(&path, functions)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("js") {
                match JavaScriptFunction::from_file(&path) {
                    Ok(js_func) => {
                        functions.push(Box::new(js_func) as Box<dyn CustomFunction>);
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to load JS function from {:?}: {}", path, e);
                    }
                }
            }
        }
        
        Ok(())
    }

    /// Auto-register all functions from the hooks directory
    pub fn auto_register(&self) -> Result<usize, Error> {
        let functions = self.load_functions()?;
        let count = functions.len();
        
        for function in functions {
            crate::register_function(function)?;
        }
        
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_js_function_parsing() {
        let js_code = r#"
            // @name: DOUBLE
            // @min_args: 1
            // @max_args: 1
            // @description: Doubles a number
            // @example: DOUBLE(5) returns 10
            
            function execute(args) {
                return args[0] * 2;
            }
        "#;

        let js_func = JavaScriptFunction::parse_js_function_internal(js_code).unwrap();
        assert_eq!(js_func.name(), "DOUBLE");
        assert_eq!(js_func.min_args(), 1);
        assert_eq!(js_func.max_args(), Some(1));
        assert_eq!(js_func.description(), Some("Doubles a number"));
        assert_eq!(js_func.example(), Some("DOUBLE(5) returns 10"));
    }

    #[test]
    fn test_js_function_execution() {
        let js_code = r#"
            // @name: TRIPLE
            // @min_args: 1
            // @max_args: 1
            
            function execute(args) {
                return args[0] * 3;
            }
        "#;

        let js_func = JavaScriptFunction::parse_js_function_internal(js_code).unwrap();
        let result = js_func.execute(vec![Value::Number(5.0)]).unwrap();
        
        match result {
            Value::Number(n) => assert!((n - 15.0).abs() < 1e-9),
            _ => panic!("Expected number result"),
        }
    }

    #[test]
    fn test_js_string_function() {
        let js_code = r#"
            // @name: REVERSE
            // @min_args: 1
            // @max_args: 1
            
            function execute(args) {
                return args[0].split('').reverse().join('');
            }
        "#;

        let js_func = JavaScriptFunction::parse_js_function_internal(js_code).unwrap();
        let result = js_func.execute(vec![Value::String("hello".to_string())]).unwrap();
        
        match result {
            Value::String(s) => assert_eq!(s, "olleh"),
            _ => panic!("Expected string result"),
        }
    }
}
