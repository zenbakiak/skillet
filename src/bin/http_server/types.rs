use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum IncludeVariables {
    All,
    None,
    Selected(Vec<String>),
}

impl<'de> Deserialize<'de> for IncludeVariables {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, Visitor};
        use std::fmt;

        struct IncludeVariablesVisitor;

        impl<'de> Visitor<'de> for IncludeVariablesVisitor {
            type Value = IncludeVariables;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a boolean or a string with comma-separated variable names")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value {
                    Ok(IncludeVariables::All)
                } else {
                    Ok(IncludeVariables::None)
                }
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let vars: Vec<String> = value
                    .split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(|s| {
                        if s.starts_with(':') {
                            s[1..].to_string()
                        } else {
                            s.to_string()
                        }
                    })
                    .collect();
                
                if vars.is_empty() {
                    Ok(IncludeVariables::None)
                } else {
                    Ok(IncludeVariables::Selected(vars))
                }
            }
        }

        deserializer.deserialize_any(IncludeVariablesVisitor)
    }
}

#[derive(Debug, Deserialize)]
pub struct EvalRequest {
    #[serde(deserialize_with = "deserialize_expression")]
    pub expression: String,
    pub arguments: Option<HashMap<String, serde_json::Value>>,
    pub output_json: Option<bool>,
    pub include_variables: Option<IncludeVariables>,
}

fn deserialize_expression<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};
    use std::fmt;

    struct ExpressionVisitor;

    impl<'de> Visitor<'de> for ExpressionVisitor {
        type Value = String;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string or array of strings")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(value.to_string())
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            let mut expressions = Vec::new();

            while let Some(expr) = seq.next_element::<String>()? {
                expressions.push(expr);
            }

            Ok(expressions.join(""))
        }
    }

    deserializer.deserialize_any(ExpressionVisitor)
}

#[derive(Debug, Serialize)]
pub struct EvalResponse {
    pub success: bool,
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<HashMap<String, serde_json::Value>>,
    pub error: Option<String>,
    pub execution_time_ms: f64,
    pub request_id: u64,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub requests_processed: u64,
    pub avg_execution_time_ms: f64,
}

#[derive(Debug, Deserialize)]
pub struct UploadJSRequest {
    pub filename: String,
    pub js_code: String,
}

#[derive(Debug, Serialize)]
pub struct UploadJSResponse {
    pub success: bool,
    pub message: String,
    pub function_name: Option<String>,
    pub validation_results: Option<ValidationResults>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateJSRequest {
    pub filename: String,
    pub js_code: String,
}

#[derive(Debug, Serialize)]
pub struct UpdateJSResponse {
    pub success: bool,
    pub message: String,
    pub function_name: Option<String>,
    pub validation_results: Option<ValidationResults>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteJSRequest {
    pub filename: String,
}

#[derive(Debug, Serialize)]
pub struct DeleteJSResponse {
    pub success: bool,
    pub message: String,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct JSFunctionInfo {
    pub filename: String,
    pub function_name: Option<String>,
    pub description: Option<String>,
    pub example: Option<String>,
    pub min_args: Option<usize>,
    pub max_args: Option<usize>,
    pub file_size: u64,
    pub last_modified: String,
    pub is_valid: bool,
    pub validation_error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListJSResponse {
    pub success: bool,
    pub functions: Vec<JSFunctionInfo>,
    pub total_count: usize,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ReloadHooksResponse {
    pub success: bool,
    pub message: String,
    pub functions_loaded: usize,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ValidationResults {
    pub syntax_valid: bool,
    pub structure_valid: bool,
    pub example_test_passed: bool,
    pub example_result: Option<String>,
    pub example_error: Option<String>,
}