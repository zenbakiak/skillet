use crate::error::Error;
use crate::types::Value;
use std::collections::HashMap;

/// Trait for custom function implementations
pub trait CustomFunction: Send + Sync {
    /// Execute the custom function with given arguments
    fn execute(&self, args: &[Value]) -> Result<Value, Error>;
    
    /// Get function name
    fn name(&self) -> &str;
    
    /// Get function description
    fn description(&self) -> &str {
        "Custom function"
    }
    
    /// Get expected argument count (None = variadic)
    fn arity(&self) -> Option<usize> {
        None
    }
    
    /// Validate arguments before execution
    fn validate_args(&self, args: &[Value]) -> Result<(), Error> {
        if let Some(expected) = self.arity() {
            if args.len() != expected {
                return Err(Error::new(
                    format!("{} expects {} arguments, got {}", 
                            self.name(), expected, args.len()),
                    None,
                ));
            }
        }
        Ok(())
    }
}

/// Trait for type converters
pub trait TypeConverter: Send + Sync {
    /// Convert a Value to the target type
    fn convert(&self, value: Value) -> Result<Value, Error>;
    
    /// Get the target type name
    fn target_type(&self) -> &str;
    
    /// Check if conversion is possible without actually converting
    fn can_convert(&self, value: &Value) -> bool;
}

/// Trait for variable resolvers
pub trait VariableResolver: Send + Sync {
    /// Resolve a variable by name
    fn resolve(&self, name: &str) -> Result<Option<Value>, Error>;
    
    /// Check if a variable exists
    fn has_variable(&self, name: &str) -> bool;
    
    /// Get all available variables
    fn get_all_variables(&self) -> HashMap<String, Value>;
}

/// Trait for method handlers
pub trait MethodHandler: Send + Sync {
    /// Check if this handler can handle the method for the given value type
    fn can_handle(&self, value: &Value, method_name: &str) -> bool;
    
    /// Execute the method
    fn execute_method(
        &self,
        value: &Value,
        method_name: &str,
        args: &[Value],
    ) -> Result<Value, Error>;
    
    /// Get list of supported methods for a value type
    fn supported_methods(&self, value: &Value) -> Vec<&str>;
}

/// Trait for expression evaluators (allows custom evaluation strategies)
pub trait ExpressionEvaluator: Send + Sync {
    /// Evaluate an expression with context
    fn evaluate(&self, expr: &crate::ast::Expr, context: &dyn EvaluationContext) -> Result<Value, Error>;
}

/// Trait for evaluation context (already partially implemented in evaluator.rs)
pub trait EvaluationContext: Send + Sync {
    /// Get a variable value
    fn get_variable(&self, name: &str) -> Option<&Value>;
    
    /// Set a variable value (for mutable contexts)
    fn set_variable(&mut self, _name: String, _value: Value) -> Result<(), Error> {
        Err(Error::new("Context does not support variable assignment", None))
    }
    
    /// Get custom function registry
    fn get_custom_registry(&self) -> Option<&std::sync::Arc<std::sync::RwLock<crate::custom::FunctionRegistry>>>;
    
    /// Clone variables for lambda evaluation
    fn clone_variables(&self) -> HashMap<String, Value>;
    
    /// Get variable resolver
    fn get_resolver(&self) -> Option<&dyn VariableResolver> {
        None
    }
}

/// Trait for pluggable data sources
pub trait DataSource: Send + Sync {
    /// Fetch data by key
    fn fetch(&self, key: &str) -> Result<Value, Error>;
    
    /// Check if data exists for key
    fn exists(&self, key: &str) -> bool;
    
    /// Get data source name/identifier
    fn name(&self) -> &str;
    
    /// Refresh/reload data (optional)
    fn refresh(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

/// Registry for extensible components
pub struct ExtensionRegistry {
    custom_functions: HashMap<String, Box<dyn CustomFunction>>,
    type_converters: HashMap<String, Box<dyn TypeConverter>>,
    method_handlers: Vec<Box<dyn MethodHandler>>,
    data_sources: HashMap<String, Box<dyn DataSource>>,
}

impl ExtensionRegistry {
    pub fn new() -> Self {
        Self {
            custom_functions: HashMap::new(),
            type_converters: HashMap::new(),
            method_handlers: Vec::new(),
            data_sources: HashMap::new(),
        }
    }
    
    /// Register a custom function
    pub fn register_function(&mut self, func: Box<dyn CustomFunction>) {
        self.custom_functions.insert(func.name().to_string(), func);
    }
    
    /// Register a type converter
    pub fn register_converter(&mut self, converter: Box<dyn TypeConverter>) {
        self.type_converters.insert(converter.target_type().to_string(), converter);
    }
    
    /// Register a method handler
    pub fn register_method_handler(&mut self, handler: Box<dyn MethodHandler>) {
        self.method_handlers.push(handler);
    }
    
    /// Register a data source
    pub fn register_data_source(&mut self, source: Box<dyn DataSource>) {
        self.data_sources.insert(source.name().to_string(), source);
    }
    
    /// Get custom function by name
    pub fn get_function(&self, name: &str) -> Option<&dyn CustomFunction> {
        self.custom_functions.get(name).map(|f| f.as_ref())
    }
    
    /// Get type converter by target type
    pub fn get_converter(&self, target_type: &str) -> Option<&dyn TypeConverter> {
        self.type_converters.get(target_type).map(|c| c.as_ref())
    }
    
    /// Find method handler for value and method
    pub fn find_method_handler(&self, value: &Value, method_name: &str) -> Option<&dyn MethodHandler> {
        self.method_handlers
            .iter()
            .find(|h| h.can_handle(value, method_name))
            .map(|h| h.as_ref())
    }
    
    /// Get data source by name
    pub fn get_data_source(&self, name: &str) -> Option<&dyn DataSource> {
        self.data_sources.get(name).map(|d| d.as_ref())
    }
}

impl Default for ExtensionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// Example implementations

/// Simple in-memory variable resolver
pub struct MemoryVariableResolver {
    variables: HashMap<String, Value>,
}

impl MemoryVariableResolver {
    pub fn new(variables: HashMap<String, Value>) -> Self {
        Self { variables }
    }
}

impl VariableResolver for MemoryVariableResolver {
    fn resolve(&self, name: &str) -> Result<Option<Value>, Error> {
        Ok(self.variables.get(name).cloned())
    }
    
    fn has_variable(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }
    
    fn get_all_variables(&self) -> HashMap<String, Value> {
        self.variables.clone()
    }
}

/// Simple constant data source
pub struct ConstantDataSource {
    name: String,
    data: HashMap<String, Value>,
}

impl ConstantDataSource {
    pub fn new(name: String, data: HashMap<String, Value>) -> Self {
        Self { name, data }
    }
}

impl DataSource for ConstantDataSource {
    fn fetch(&self, key: &str) -> Result<Value, Error> {
        self.data
            .get(key)
            .cloned()
            .ok_or_else(|| Error::new(format!("Key '{}' not found in data source '{}'", key, self.name), None))
    }
    
    fn exists(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }
    
    fn name(&self) -> &str {
        &self.name
    }
}