use crate::types::Value;
use crate::error::Error;
use super::arithmetic;
use super::logical;
use super::string;
use super::array;
use super::datetime;
use super::financial;
use super::statistical;

pub fn exec_builtin(name: &str, args: &[Value]) -> Result<Value, Error> {
    // Try arithmetic functions first
    if let Ok(result) = arithmetic::exec_arithmetic(name, args) {
        return Ok(result);
    }
    
    // Try logical functions
    if let Ok(result) = logical::exec_logical(name, args) {
        return Ok(result);
    }
    
    // Try string functions
    if let Ok(result) = string::exec_string(name, args) {
        return Ok(result);
    }
    
    // Try array functions
    if let Ok(result) = array::exec_array(name, args) {
        return Ok(result);
    }
    
    // Try datetime functions
    if let Ok(result) = datetime::exec_datetime(name, args) {
        return Ok(result);
    }
    
    // Try financial functions
    if let Ok(result) = financial::exec_financial(name, args) {
        return Ok(result);
    }
    
    // Try statistical functions
    if let Ok(result) = statistical::exec_statistical(name, args) {
        return Ok(result);
    }
    
    // Handle remaining functions not yet modularized
    match name {
        
        // SUMIF/AVGIF/COUNTIF handled in FunctionCall branch to preserve lambda expr
        _ => Err(Error::new(format!("Unknown function: {}", name), None)),
    }
}