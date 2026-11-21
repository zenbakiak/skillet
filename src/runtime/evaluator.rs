use crate::ast::{BinaryOp, Expr, UnaryOp};
use crate::custom::{execute_checked, FunctionRegistry};
use crate::error::Error;
use crate::limits::{self, charge_value, check_value};
use crate::runtime::{
    function_dispatch::exec_builtin_fast,
    method_calls::exec_method_in,
    type_casting::cast_value,
    utils::{clamp_index, slice_items},
};
use crate::types::Value;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Evaluation context that provides access to variables and custom functions
pub trait EvaluationContext {
    fn get_variable(&self, name: &str) -> Option<&Value>;
    fn get_custom_registry(&self) -> Option<&Arc<RwLock<FunctionRegistry>>>;
    fn clone_variables(&self) -> HashMap<String, Value>;
}

/// Empty context for basic evaluation without variables
pub struct EmptyContext;

impl EvaluationContext for EmptyContext {
    fn get_variable(&self, _name: &str) -> Option<&Value> {
        None
    }

    fn get_custom_registry(&self) -> Option<&Arc<RwLock<FunctionRegistry>>> {
        None
    }

    fn clone_variables(&self) -> HashMap<String, Value> {
        HashMap::new()
    }
}

/// Variable context with optional custom function support
pub struct VariableContext<'a> {
    variables: Cow<'a, HashMap<String, Value>>,
    custom_registry: Option<&'a Arc<RwLock<FunctionRegistry>>>,
}

impl<'a> VariableContext<'a> {
    pub fn new(vars: &'a HashMap<String, Value>) -> Self {
        Self {
            variables: Cow::Borrowed(vars),
            custom_registry: None,
        }
    }

    pub fn with_custom(
        vars: &'a HashMap<String, Value>,
        registry: &'a Arc<RwLock<FunctionRegistry>>,
    ) -> Self {
        Self {
            variables: Cow::Borrowed(vars),
            custom_registry: Some(registry),
        }
    }

    pub fn with_owned(vars: HashMap<String, Value>) -> Self {
        Self {
            variables: Cow::Owned(vars),
            custom_registry: None,
        }
    }

    /// Make variables mutable, cloning if necessary
    pub fn make_mut(&mut self) -> &mut HashMap<String, Value> {
        self.variables.to_mut()
    }

    /// Consume the context and return the owned variables HashMap
    pub fn into_variables(self) -> HashMap<String, Value> {
        self.variables.into_owned()
    }
}

impl<'a> EvaluationContext for VariableContext<'a> {
    fn get_variable(&self, name: &str) -> Option<&Value> {
        self.variables.get(name)
    }

    fn get_custom_registry(&self) -> Option<&Arc<RwLock<FunctionRegistry>>> {
        self.custom_registry
    }

    fn clone_variables(&self) -> HashMap<String, Value> {
        self.variables.as_ref().clone()
    }
}

/// Local bindings (lambda item/accumulator) layered over a borrowed parent context.
///
/// Lookups check the locals first, then the parent, so lambdas never copy the caller's
/// variables (BL-030). `registry` is explicit because some call sites deliberately do not
/// expose custom functions (plain method arguments, `FILTER(...)`-style builtins).
pub(crate) struct Scope<'a> {
    parent: &'a dyn EvaluationContext,
    locals: Vec<(String, Value)>,
    registry: Option<&'a Arc<RwLock<FunctionRegistry>>>,
}

impl<'a> Scope<'a> {
    pub(crate) fn new(
        parent: &'a dyn EvaluationContext,
        registry: Option<&'a Arc<RwLock<FunctionRegistry>>>,
    ) -> Self {
        Self {
            parent,
            locals: Vec::new(),
            registry,
        }
    }

    /// Bind `name` to `value`, replacing an earlier local binding of the same name.
    pub(crate) fn set(&mut self, name: &str, value: Value) {
        match self.locals.iter_mut().find(|(n, _)| n == name) {
            Some(slot) => slot.1 = value,
            None => self.locals.push((name.to_string(), value)),
        }
    }
}

impl EvaluationContext for Scope<'_> {
    fn get_variable(&self, name: &str) -> Option<&Value> {
        match self.locals.iter().find(|(n, _)| n == name) {
            Some((_, v)) => Some(v),
            None => self.parent.get_variable(name),
        }
    }

    fn get_custom_registry(&self) -> Option<&Arc<RwLock<FunctionRegistry>>> {
        self.registry
    }

    fn clone_variables(&self) -> HashMap<String, Value> {
        let mut vars = self.parent.clone_variables();
        for (n, v) in &self.locals {
            vars.insert(n.clone(), v.clone());
        }
        vars
    }
}

/// Evaluate a plain (non-lambda) method argument: caller's variables, no custom functions.
pub(crate) fn eval_arg(expr: &Expr, ctx: &dyn EvaluationContext) -> Result<Value, Error> {
    Evaluator::eval(expr, &Scope::new(ctx, None))
}

fn missing_variable(name: &str) -> Error {
    Error::new(format!("Missing variable: :{}", name), None)
}

/// Unified evaluator that handles all expression types efficiently
pub struct Evaluator;

impl Evaluator {
    /// Evaluate expression with any context type
    pub fn eval<C: EvaluationContext>(expr: &Expr, context: &C) -> Result<Value, Error> {
        match expr {
            Expr::Number(n) => Ok(Value::Number(*n)),
            Expr::StringLit(s) => Ok(Value::String(s.clone())),
            Expr::Null => Ok(Value::Null),

            Expr::Unary(op, e) => {
                let v = Self::eval_operand(e, context)?;
                limits::tick()?;
                Self::eval_unary_op(*op, &v)
            }

            Expr::Binary(l, op, r) => {
                let a = Self::eval_operand(l, context)?;
                let b = Self::eval_operand(r, context)?;
                limits::tick()?;
                Self::eval_binary_op(*op, &a, &b)
            }

            // A read that copies is charged the cost of the copy, before it is made
            // (ADR-0011 amendment 2). Borrowed reads (`eval_operand`) only tick.
            Expr::Variable(name) => {
                let v = context
                    .get_variable(name)
                    .ok_or_else(|| missing_variable(name))?;
                charge_value(v)?;
                Ok(v.clone())
            }

            Expr::PropertyAccess { target, property } => Self::with_target(target, context, |v| {
                Self::eval_property_access(v, property, false).and_then(check_value)
            }),

            Expr::SafePropertyAccess { target, property } => {
                Self::with_target(target, context, |v| {
                    Self::eval_property_access(v, property, true).and_then(check_value)
                })
            }

            Expr::SafeMethodCall { target, name, args } => {
                Self::with_target(target, context, |recv| {
                    if matches!(recv, Value::Null) {
                        return Ok(Value::Null);
                    }
                    exec_method_in(name, false, recv, args, context).and_then(check_value)
                })
            }

            Expr::Array(items) => {
                let mut out = Vec::with_capacity(items.len());
                for e in items {
                    out.push(Self::eval(e, context)?);
                }
                check_value(Value::Array(out))
            }

            Expr::ObjectLiteral(pairs) => Self::eval_object_literal(pairs, context),

            Expr::TypeCast { expr, ty } => {
                let v = Self::eval(expr, context)?;
                cast_value(v, ty).and_then(check_value)
            }

            Expr::Index { target, index } => Self::with_target(target, context, |recv| {
                let idx_v = Self::eval(index, context)?;
                let idx = idx_v
                    .as_number()
                    .ok_or_else(|| Error::new("Index must be number", None))?
                    as isize;
                match recv {
                    Value::Array(items) => match clamp_index(items.len(), idx) {
                        Some(i) => check_value(items[i].clone()),
                        None => Err(Error::new("Index out of bounds", None)),
                    },
                    _ => Err(Error::new("Index on non-array", None)),
                }
            }),

            Expr::Slice { target, start, end } => {
                Self::with_target(target, context, |recv| match recv {
                    Value::Array(items) => {
                        let start_val =
                            start.as_ref().map(|e| Self::eval(e, context)).transpose()?;
                        let end_val = end.as_ref().map(|e| Self::eval(e, context)).transpose()?;
                        slice_items(items, start_val, end_val).and_then(check_value)
                    }
                    _ => Err(Error::new("Slice on non-array", None)),
                })
            }

            Expr::FunctionCall { name, args } => {
                let v = Self::eval_function_call(name, args, context)?;
                if Self::is_prepaid_builtin(name, context) {
                    limits::check_prepaid(v)
                } else {
                    check_value(v)
                }
            }

            Expr::MethodCall {
                target,
                name,
                args,
                predicate,
            } => Self::with_target(target, context, |recv| {
                exec_method_in(name, *predicate, recv, args, context).and_then(check_value)
            }),

            Expr::Spread(_) => Err(Error::new("Spread not allowed here", None)),

            Expr::Assignment { variable: _, value } => {
                // For now, return the value - assignments need mutable context
                Self::eval(value, context)
            }

            Expr::Sequence(exprs) => {
                let mut last_result = Value::Null;
                for expr in exprs {
                    last_result = Self::eval(expr, context)?;
                }
                Ok(last_result)
            }
        }
    }

    /// Evaluate an operator operand or a method/property target. A plain variable is borrowed
    /// (no copy, no charge), so `:s == :s` on a large string compares in place instead of
    /// cloning it twice. The borrowed read does not tick by itself: the operation that uses it
    /// does (the operator's tick, or the method/property result charge), one tick per use.
    fn eval_operand<'c, C: EvaluationContext>(
        expr: &Expr,
        context: &'c C,
    ) -> Result<Cow<'c, Value>, Error> {
        match expr {
            Expr::Variable(name) => context
                .get_variable(name)
                .map(Cow::Borrowed)
                .ok_or_else(|| missing_variable(name)),
            _ => Self::eval(expr, context).map(Cow::Owned),
        }
    }

    /// Evaluate unary operations
    fn eval_unary_op(op: UnaryOp, v: &Value) -> Result<Value, Error> {
        match op {
            UnaryOp::Plus => {
                Ok(Value::Number(v.as_number().ok_or_else(|| {
                    Error::new("Unary '+' on non-number", None)
                })?))
            }
            UnaryOp::Minus => {
                Ok(Value::Number(-v.as_number().ok_or_else(|| {
                    Error::new("Unary '-' on non-number", None)
                })?))
            }
            UnaryOp::Not => {
                Ok(Value::Boolean(!v.as_bool().ok_or_else(|| {
                    Error::new("Unary '!' on non-boolean", None)
                })?))
            }
        }
    }

    /// Evaluate binary operations
    fn eval_binary_op(op: BinaryOp, a: &Value, b: &Value) -> Result<Value, Error> {
        match op {
            BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Mod
            | BinaryOp::Pow => {
                let an = a
                    .as_number()
                    .ok_or_else(|| Error::new("Arithmetic op on non-number", None))?;
                let bn = b
                    .as_number()
                    .ok_or_else(|| Error::new("Arithmetic op on non-number", None))?;
                Ok(Value::Number(match op {
                    BinaryOp::Add => an + bn,
                    BinaryOp::Sub => an - bn,
                    BinaryOp::Mul => an * bn,
                    BinaryOp::Div => an / bn,
                    BinaryOp::Mod => an % bn,
                    BinaryOp::Pow => an.powf(bn),
                    _ => unreachable!(),
                }))
            }
            BinaryOp::Eq
            | BinaryOp::Ne
            | BinaryOp::Lt
            | BinaryOp::Le
            | BinaryOp::Gt
            | BinaryOp::Ge => {
                let result = match (a, b) {
                    (Value::Number(x), Value::Number(y)) => match op {
                        BinaryOp::Eq => x == y,
                        BinaryOp::Ne => x != y,
                        BinaryOp::Lt => x < y,
                        BinaryOp::Le => x <= y,
                        BinaryOp::Gt => x > y,
                        BinaryOp::Ge => x >= y,
                        _ => unreachable!(),
                    },
                    (Value::String(x), Value::String(y)) => match op {
                        BinaryOp::Eq => x == y,
                        BinaryOp::Ne => x != y,
                        BinaryOp::Lt => x < y,
                        BinaryOp::Le => x <= y,
                        BinaryOp::Gt => x > y,
                        BinaryOp::Ge => x >= y,
                        _ => unreachable!(),
                    },
                    (Value::Boolean(x), Value::Boolean(y)) => match op {
                        BinaryOp::Eq => x == y,
                        BinaryOp::Ne => x != y,
                        _ => false,
                    },
                    _ => match op {
                        BinaryOp::Eq => false,
                        BinaryOp::Ne => true,
                        _ => return Err(Error::new("Comparison of incompatible types", None)),
                    },
                };
                Ok(Value::Boolean(result))
            }
            BinaryOp::And | BinaryOp::Or => {
                let ab = a
                    .as_bool()
                    .ok_or_else(|| Error::new("Logical op on non-boolean", None))?;
                let bb = b
                    .as_bool()
                    .ok_or_else(|| Error::new("Logical op on non-boolean", None))?;
                Ok(Value::Boolean(match op {
                    BinaryOp::And => ab && bb,
                    BinaryOp::Or => ab || bb,
                    _ => unreachable!(),
                }))
            }
        }
    }

    /// Run `f` on the value of a method/property target. A plain variable is borrowed
    /// instead of cloned, so `:rows.length()` inside a lambda does not copy `:rows`.
    /// `{"k": v, …}`: a `Json` object. Escaping can grow the text 6x (`\u0001`), so its exact
    /// size is charged before it is built; that charge is the result's cost (not checked again).
    /// Out of line so this cold path does not change code layout of the hot `eval` match
    /// (measured: inline, `lambda_scope` `:nums.map(:x * 1)` was +6%).
    #[inline(never)]
    fn eval_object_literal<C: EvaluationContext>(
        pairs: &[(String, Expr)],
        context: &C,
    ) -> Result<Value, Error> {
        let mut json_map = serde_json::Map::new();
        for (key, value_expr) in pairs {
            let value = Self::eval(value_expr, context)?;
            let json_value = Self::value_to_json(&value)?;
            json_map.insert(key.clone(), json_value);
        }
        let json_obj = serde_json::Value::Object(json_map);
        limits::charge_json_text(&json_obj)?;
        let json_str = serde_json::to_string(&json_obj)
            .map_err(|e| Error::new(format!("Failed to serialize object: {}", e), None))?;
        Ok(Value::Json(json_str))
    }

    fn with_target<C: EvaluationContext>(
        target: &Expr,
        context: &C,
        f: impl FnOnce(&Value) -> Result<Value, Error>,
    ) -> Result<Value, Error> {
        f(Self::eval_operand(target, context)?.as_ref())
    }

    /// Evaluate property access (both safe and unsafe)
    fn eval_property_access(
        target_value: &Value,
        property: &str,
        safe: bool,
    ) -> Result<Value, Error> {
        match target_value {
            Value::Json(json_str) => {
                limits::charge_json_parse(json_str)?;
                let parsed: serde_json::Value = serde_json::from_str(json_str)
                    .map_err(|e| Error::new(format!("Invalid JSON: {}", e), None))?;
                if let Some(prop_value) = parsed.get(property) {
                    crate::json_to_value(prop_value.clone())
                } else if safe {
                    Ok(Value::Null)
                } else {
                    Err(Error::new(
                        format!("Property '{}' not found in JSON object", property),
                        None,
                    ))
                }
            }
            Value::Null if safe => Ok(Value::Null),
            _ => Err(Error::new("Property access requires JSON object", None)),
        }
    }

    /// Builtins that charge their own result before building it (ADR-0011 amendment 2); their
    /// result is only depth-checked. A custom function of the same name is charged as usual.
    fn is_prepaid_builtin<C: EvaluationContext>(name: &str, context: &C) -> bool {
        matches!(name, "MERGE" | "CONCAT")
            && !context
                .get_custom_registry()
                .and_then(|r| r.read().ok().map(|reg| reg.has_function(name)))
                .unwrap_or(false)
    }

    /// Evaluate function calls with optimized dispatch
    fn eval_function_call<C: EvaluationContext>(
        name: &str,
        args: &[Expr],
        context: &C,
    ) -> Result<Value, Error> {
        // Handle special functions first
        match name {
            "__TERNARY__" => {
                if args.len() != 3 {
                    return Err(Error::new("Ternary expects 3 args", None));
                }
                let cond = Self::eval(&args[0], context)?
                    .as_bool()
                    .ok_or_else(|| Error::new("Ternary condition must be boolean", None))?;
                return if cond {
                    Self::eval(&args[1], context)
                } else {
                    Self::eval(&args[2], context)
                };
            }
            "__CONST_TRUE__" => return Ok(Value::Boolean(true)),
            "__CONST_FALSE__" => return Ok(Value::Boolean(false)),
            _ => {}
        }

        // Check custom functions first. Take a handle and release the registry lock before
        // evaluating arguments or running the function, so a slow function doesn't block
        // writers (registration, reload) and nested custom calls don't re-lock.
        let custom = context
            .get_custom_registry()
            .and_then(|registry| registry.read().ok()?.get_shared(name));
        if let Some(function) = custom {
            let mut ev_args = Vec::new();
            for a in args {
                match a {
                    Expr::Spread(inner) => {
                        let v = Self::eval(inner, context)?;
                        if let Value::Array(items) = v {
                            ev_args.extend(items);
                        } else {
                            return Err(Error::new("Spread expects array", None));
                        }
                    }
                    _ => {
                        let val = Self::eval(a, context)?;
                        ev_args.push(val);
                    }
                }
            }
            return execute_checked(function.as_ref(), name, ev_args);
        }

        // Handle higher-order functions
        match name {
            "FILTER" => Self::eval_filter(args, context),
            "FIND" => Self::eval_find(args, context),
            "MAP" => Self::eval_map(args, context),
            "REDUCE" => Self::eval_reduce(args, context),
            "SUMIF" => Self::eval_sumif(args, context),
            "AVGIF" => Self::eval_avgif(args, context),
            "COUNTIF" => Self::eval_countif(args, context),
            "JQ" => {
                if args.len() != 2 {
                    return Err(Error::new(
                        "JQ expects exactly 2 arguments: json_data, jsonpath_expression",
                        None,
                    ));
                }

                let json_data = Self::eval(&args[0], context)?;
                let path_expr = Self::eval(&args[1], context)?;

                let path = match path_expr {
                    Value::String(s) => s,
                    _ => return Err(Error::new("JQ second argument must be a string", None)),
                };

                if !crate::runtime::jsonpath::is_jsonpath(&path) {
                    return Err(Error::new(
                        "JQ second argument must be a valid JSONPath expression starting with $",
                        None,
                    ));
                }

                crate::runtime::jsonpath::apply_jsonpath(&json_data, &path)
            }
            _ => {
                // Evaluate arguments for built-in functions
                let mut ev_args = Vec::new();
                for a in args {
                    match a {
                        Expr::Spread(inner) => {
                            let v = Self::eval(inner, context)?;
                            if let Value::Array(items) = v {
                                ev_args.extend(items);
                            } else {
                                return Err(Error::new("Spread expects array", None));
                            }
                        }
                        _ => {
                            let val = Self::eval(a, context)?;
                            ev_args.push(val);
                        }
                    }
                }
                exec_builtin_fast(name, &ev_args)
            }
        }
    }

    /// Helper for higher-order functions - these need access to context for lambda evaluation
    fn eval_filter<C: EvaluationContext>(args: &[Expr], context: &C) -> Result<Value, Error> {
        if args.len() < 2 {
            return Err(Error::new("FILTER expects (array, expr)", None));
        }
        let arr_v = Self::eval(&args[0], context)?;
        let lambda = &args[1];
        let param_name = if args.len() > 2 {
            if let Value::String(s) = Self::eval(&args[2], context)? {
                s
            } else {
                "x".into()
            }
        } else {
            "x".into()
        };

        match arr_v {
            Value::Array(items) => {
                let mut out = Vec::with_capacity(items.len());
                let mut scope = Scope::new(context, None);
                for it in items {
                    charge_value(&it)?;
                    scope.set(&param_name, it.clone());
                    let matches = matches!(Self::eval(lambda, &scope)?, Value::Boolean(true));
                    if matches {
                        out.push(it);
                    }
                }
                Ok(Value::Array(out))
            }
            _ => Err(Error::new("FILTER first arg must be array", None)),
        }
    }

    fn eval_find<C: EvaluationContext>(args: &[Expr], context: &C) -> Result<Value, Error> {
        if args.len() < 2 {
            return Err(Error::new("FIND expects (array, expr)", None));
        }
        let arr_v = Self::eval(&args[0], context)?;
        let lambda = &args[1];
        let param_name = if args.len() > 2 {
            if let Value::String(s) = Self::eval(&args[2], context)? {
                s
            } else {
                "x".into()
            }
        } else {
            "x".into()
        };

        match arr_v {
            Value::Array(items) => {
                let mut scope = Scope::new(context, None);
                for it in items {
                    charge_value(&it)?;
                    scope.set(&param_name, it.clone());
                    let matches = matches!(Self::eval(lambda, &scope)?, Value::Boolean(true));
                    if matches {
                        return Ok(it);
                    }
                }
                Ok(Value::Null)
            }
            _ => Err(Error::new("FIND first arg must be array", None)),
        }
    }

    fn eval_map<C: EvaluationContext>(args: &[Expr], context: &C) -> Result<Value, Error> {
        if args.len() < 2 {
            return Err(Error::new("MAP expects (array, expr)", None));
        }
        let arr_v = Self::eval(&args[0], context)?;
        let lambda = &args[1];
        let param_name = if args.len() > 2 {
            if let Value::String(s) = Self::eval(&args[2], context)? {
                s
            } else {
                "x".into()
            }
        } else {
            "x".into()
        };

        match arr_v {
            Value::Array(items) => {
                let mut out = Vec::with_capacity(items.len());
                let mut scope = Scope::new(context, None);
                for it in items {
                    charge_value(&it)?;
                    scope.set(&param_name, it);
                    let result = Self::eval(lambda, &scope)?;
                    out.push(result);
                }
                Ok(Value::Array(out))
            }
            _ => Err(Error::new("MAP first arg must be array", None)),
        }
    }

    fn eval_reduce<C: EvaluationContext>(args: &[Expr], context: &C) -> Result<Value, Error> {
        if args.len() < 3 {
            return Err(Error::new("REDUCE expects (array, expr, initial)", None));
        }
        let arr_v = Self::eval(&args[0], context)?;
        let lambda = &args[1];
        let mut acc = Self::eval(&args[2], context)?;
        let val_param = if args.len() > 3 {
            if let Value::String(s) = Self::eval(&args[3], context)? {
                s
            } else {
                "x".into()
            }
        } else {
            "x".into()
        };
        let acc_param = if args.len() > 4 {
            if let Value::String(s) = Self::eval(&args[4], context)? {
                s
            } else {
                "acc".into()
            }
        } else {
            "acc".into()
        };

        match arr_v {
            Value::Array(items) => {
                let mut scope = Scope::new(context, None);
                for it in items {
                    charge_value(&it)?;
                    scope.set(&val_param, it);
                    scope.set(&acc_param, acc);
                    acc = Self::eval(lambda, &scope)?;
                }
                Ok(acc)
            }
            _ => Err(Error::new("REDUCE first arg must be array", None)),
        }
    }

    fn eval_sumif<C: EvaluationContext>(args: &[Expr], context: &C) -> Result<Value, Error> {
        if args.len() < 2 || args.len() > 3 {
            return Err(Error::new(
                "SUMIF expects (array, criteria) or (array, criteria, sum_array)",
                None,
            ));
        }
        let arr_v = Self::eval(&args[0], context)?;
        let criteria_expr = &args[1];
        let sum_array = if args.len() == 3 {
            Some(Self::eval(&args[2], context)?)
        } else {
            None
        };

        // First try to evaluate the criteria as a static value (Excel-style string criteria)
        if let Ok(criteria_value) = Self::eval(criteria_expr, context) {
            if let Value::String(_) | Value::Number(_) = criteria_value {
                // Excel-style criteria - use string/numeric comparison logic
                return Self::eval_sumif_excel_style(
                    &arr_v,
                    &criteria_value,
                    sum_array.as_ref().unwrap_or(&arr_v),
                );
            }
        }

        // If that fails, fall back to lambda-based evaluation (existing behavior)
        if args.len() != 2 {
            return Err(Error::new(
                "Lambda-style SUMIF expects exactly (array, expr)",
                None,
            ));
        }

        match arr_v {
            Value::Array(items) => {
                let mut acc = 0.0;
                let mut scope = Scope::new(context, None);
                for it in items {
                    charge_value(&it)?;
                    scope.set("x", it.clone());
                    let matches =
                        matches!(Self::eval(criteria_expr, &scope)?, Value::Boolean(true));
                    if matches {
                        match it {
                            Value::Number(n) | Value::Currency(n) => acc += n,
                            _ => {}
                        }
                    }
                }
                Ok(Value::Number(acc))
            }
            _ => Err(Error::new("SUMIF first arg must be array", None)),
        }
    }

    fn eval_sumif_excel_style(
        range: &Value,
        criteria: &Value,
        sum_range: &Value,
    ) -> Result<Value, Error> {
        fn meets_criteria(value: &Value, criteria: &Value) -> bool {
            match criteria {
                Value::String(crit) => {
                    if let Some(stripped) = crit.strip_prefix(">=") {
                        if let Ok(threshold) = stripped.parse::<f64>() {
                            match value {
                                Value::Number(n) => *n >= threshold,
                                Value::Currency(n) => *n >= threshold,
                                _ => false,
                            }
                        } else {
                            false
                        }
                    } else if let Some(stripped) = crit.strip_prefix("<=") {
                        if let Ok(threshold) = stripped.parse::<f64>() {
                            match value {
                                Value::Number(n) => *n <= threshold,
                                Value::Currency(n) => *n <= threshold,
                                _ => false,
                            }
                        } else {
                            false
                        }
                    } else if let Some(stripped) = crit.strip_prefix("<>") {
                        if let Ok(threshold) = stripped.parse::<f64>() {
                            match value {
                                Value::Number(n) => *n != threshold,
                                Value::Currency(n) => *n != threshold,
                                _ => true,
                            }
                        } else {
                            match value {
                                Value::String(s) => s != stripped,
                                _ => true,
                            }
                        }
                    } else if let Some(stripped) = crit.strip_prefix('>') {
                        if let Ok(threshold) = stripped.parse::<f64>() {
                            match value {
                                Value::Number(n) => *n > threshold,
                                Value::Currency(n) => *n > threshold,
                                _ => false,
                            }
                        } else {
                            false
                        }
                    } else if let Some(stripped) = crit.strip_prefix('<') {
                        if let Ok(threshold) = stripped.parse::<f64>() {
                            match value {
                                Value::Number(n) => *n < threshold,
                                Value::Currency(n) => *n < threshold,
                                _ => false,
                            }
                        } else {
                            false
                        }
                    } else if let Some(stripped) = crit.strip_prefix('=') {
                        if let Ok(threshold) = stripped.parse::<f64>() {
                            match value {
                                Value::Number(n) => *n == threshold,
                                Value::Currency(n) => *n == threshold,
                                _ => false,
                            }
                        } else {
                            match value {
                                Value::String(s) => s == stripped,
                                _ => false,
                            }
                        }
                    } else if let Ok(threshold) = crit.parse::<f64>() {
                        match value {
                            Value::Number(n) => *n == threshold,
                            Value::Currency(n) => *n == threshold,
                            _ => false,
                        }
                    } else {
                        match value {
                            Value::String(s) => s == crit,
                            _ => false,
                        }
                    }
                }
                Value::Number(threshold) => match value {
                    Value::Number(n) => *n == *threshold,
                    Value::Currency(n) => *n == *threshold,
                    _ => false,
                },
                _ => false,
            }
        }

        fn sum_if_helper(range_val: &Value, sum_val: &Value, criteria: &Value) -> f64 {
            match (range_val, sum_val) {
                (Value::Array(range_items), Value::Array(sum_items)) => {
                    let mut acc = 0.0;
                    let min_len = std::cmp::min(range_items.len(), sum_items.len());
                    for i in 0..min_len {
                        if meets_criteria(&range_items[i], criteria) {
                            match &sum_items[i] {
                                Value::Number(n) => acc += *n,
                                Value::Currency(n) => acc += *n,
                                _ => {}
                            }
                        }
                    }
                    acc
                }
                (range_val, sum_val) => {
                    if meets_criteria(range_val, criteria) {
                        match sum_val {
                            Value::Number(n) => *n,
                            Value::Currency(n) => *n,
                            _ => 0.0,
                        }
                    } else {
                        0.0
                    }
                }
            }
        }

        let result = sum_if_helper(range, sum_range, criteria);
        Ok(Value::Number(result))
    }

    fn eval_avgif<C: EvaluationContext>(args: &[Expr], context: &C) -> Result<Value, Error> {
        if args.len() != 2 {
            return Err(Error::new("AVGIF expects (array, expr)", None));
        }
        let arr_v = Self::eval(&args[0], context)?;
        let lambda = &args[1];

        match arr_v {
            Value::Array(items) => {
                let mut acc = 0.0;
                let mut count = 0usize;
                let mut scope = Scope::new(context, None);
                for it in items {
                    charge_value(&it)?;
                    scope.set("x", it.clone());
                    let matches = matches!(Self::eval(lambda, &scope)?, Value::Boolean(true));
                    if matches {
                        match it {
                            Value::Number(n) | Value::Currency(n) => {
                                acc += n;
                                count += 1;
                            }
                            _ => {}
                        }
                    }
                }
                Ok(Value::Number(if count == 0 {
                    0.0
                } else {
                    acc / count as f64
                }))
            }
            _ => Err(Error::new("AVGIF first arg must be array", None)),
        }
    }

    fn eval_countif<C: EvaluationContext>(args: &[Expr], context: &C) -> Result<Value, Error> {
        if args.len() != 2 {
            return Err(Error::new("COUNTIF expects (array, expr)", None));
        }
        let arr_v = Self::eval(&args[0], context)?;
        let lambda = &args[1];

        match arr_v {
            Value::Array(items) => {
                let mut count = 0usize;
                let mut scope = Scope::new(context, None);
                for it in items {
                    charge_value(&it)?;
                    scope.set("x", it);
                    let matches = matches!(Self::eval(lambda, &scope)?, Value::Boolean(true));
                    if matches {
                        count += 1;
                    }
                }
                Ok(Value::Number(count as f64))
            }
            _ => Err(Error::new("COUNTIF first arg must be array", None)),
        }
    }

    /// Helper to convert Value to JSON
    fn value_to_json(value: &Value) -> Result<serde_json::Value, Error> {
        match value {
            Value::Number(n) => Ok(serde_json::json!(n)),
            Value::String(s) => Ok(serde_json::json!(s)),
            Value::Boolean(b) => Ok(serde_json::json!(b)),
            Value::Currency(c) => Ok(serde_json::json!(c)),
            Value::DateTime(dt) => Ok(serde_json::json!(dt)),
            Value::Null => Ok(serde_json::json!(null)),
            Value::Array(arr) => {
                let mut json_arr = Vec::new();
                for item in arr {
                    json_arr.push(Self::value_to_json(item)?);
                }
                Ok(serde_json::Value::Array(json_arr))
            }
            Value::Json(s) => {
                limits::charge_json_parse(s)?;
                serde_json::from_str(s)
                    .map_err(|e| Error::new(format!("Invalid JSON: {}", e), None))
            }
        }
    }
}

// Convenience functions for backward compatibility
pub fn eval(expr: &Expr) -> Result<Value, Error> {
    limits::evaluation(|| Evaluator::eval(expr, &EmptyContext))
}

pub fn eval_with_vars(expr: &Expr, vars: &HashMap<String, Value>) -> Result<Value, Error> {
    limits::evaluation(|| Evaluator::eval(expr, &VariableContext::new(vars)))
}

pub fn eval_with_vars_and_custom(
    expr: &Expr,
    vars: &HashMap<String, Value>,
    custom_registry: &Arc<RwLock<FunctionRegistry>>,
) -> Result<Value, Error> {
    limits::evaluation(|| {
        Evaluator::eval(expr, &VariableContext::with_custom(vars, custom_registry))
    })
}

/// Evaluate with support for assignments and sequences
pub fn eval_with_assignments(expr: &Expr, vars: &HashMap<String, Value>) -> Result<Value, Error> {
    limits::evaluation(|| {
        let mut context = VariableContext::with_owned(vars.clone());
        eval_with_assignments_context(expr, &mut context)
    })
}

/// Evaluate with support for assignments and sequences, returning both result and variable context
pub fn eval_with_assignments_and_context(
    expr: &Expr,
    vars: &HashMap<String, Value>,
) -> Result<(Value, HashMap<String, Value>), Error> {
    limits::evaluation(|| {
        let mut context = VariableContext::with_owned(vars.clone());
        let result = eval_with_assignments_context(expr, &mut context)?;
        Ok((result, context.into_variables()))
    })
}

fn eval_with_assignments_context(
    expr: &Expr,
    context: &mut VariableContext,
) -> Result<Value, Error> {
    match expr {
        Expr::Assignment { variable, value } => {
            let result = Evaluator::eval(value, context)?;
            charge_value(&result)?; // the stored clone
            context.make_mut().insert(variable.clone(), result.clone());
            Ok(result)
        }
        Expr::Sequence(exprs) => {
            let mut last_result = Value::Null;
            for expr in exprs {
                last_result = eval_with_assignments_context(expr, context)?;
            }
            Ok(last_result)
        }
        // For all other expressions, delegate to unified evaluator
        _ => Evaluator::eval(expr, context),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser;

    /// Context that fails the test if anything copies the whole variable map (BL-030).
    struct NoCopyContext(HashMap<String, Value>);

    impl EvaluationContext for NoCopyContext {
        fn get_variable(&self, name: &str) -> Option<&Value> {
            self.0.get(name)
        }
        fn get_custom_registry(&self) -> Option<&Arc<RwLock<FunctionRegistry>>> {
            None
        }
        fn clone_variables(&self) -> HashMap<String, Value> {
            panic!("variable map copied")
        }
    }

    fn eval_no_copy(src: &str) -> Result<Value, Error> {
        let mut vars = HashMap::new();
        vars.insert(
            "nums".to_string(),
            Value::Array(vec![Value::Number(-1.5), Value::Number(2.0)]),
        );
        vars.insert("x".to_string(), Value::Number(100.0));
        let expr = Parser::new(src).parse()?;
        Evaluator::eval(&expr, &NoCopyContext(vars))
    }

    #[test]
    fn lambdas_and_inner_calls_do_not_copy_variables() {
        let cases = [
            (":nums.map(:x.abs()).sum()", Value::Number(3.5)),
            (":nums.map(:x / :nums.length()).sum()", Value::Number(0.25)),
            (":nums.filter(:x.round(0) > 0).length()", Value::Number(1.0)),
            (":nums.reduce(:acc + :x.abs(), 0)", Value::Number(3.5)),
            ("MAP(:nums, :x.abs()).sum()", Value::Number(3.5)),
            ("REDUCE(:nums, :acc + :x, 0)", Value::Number(0.5)),
            ("COUNTIF(:nums, :x.abs() > 1)", Value::Number(2.0)),
            ("FIND(:nums, :x > 0)", Value::Number(2.0)),
        ];
        for (src, want) in cases {
            assert_eq!(eval_no_copy(src).ok(), Some(want), "{}", src);
        }
    }

    #[test]
    fn scope_locals_shadow_parent_and_rebind() {
        // `:x` is a top-level variable (100) shadowed by the item inside the lambda only.
        assert_eq!(
            eval_no_copy(":nums.map(:x).sum() + :x").ok(),
            Some(Value::Number(100.5))
        );
        // Same name for item and accumulator: accumulator binding wins, as before BL-030.
        assert_eq!(
            eval_no_copy(":nums.reduce(:x, 7, \"x\", \"x\")").ok(),
            Some(Value::Number(7.0))
        );
    }
}
