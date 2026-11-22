use crate::ast::{BinaryOp, Expr, UnaryOp};
use crate::custom::{execute_checked, FunctionRegistry};
use crate::error::Error;
use crate::limits::{self, charge_value, check_value};
use crate::runtime::{
    criteria::{aggregate, range_items, reads_item, Aggregate, Criterion},
    function_dispatch::exec_builtin_fast,
    logical,
    method_calls::exec_method_in,
    type_casting::cast_value,
    utils::{clamp_index, coerce_to_number, slice_items},
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
/// variables (BL-030). Custom functions are the parent's: every sub-expression sees the same
/// registry as the entry point (ADR-0016).
pub(crate) struct Scope<'a> {
    parent: &'a dyn EvaluationContext,
    locals: Vec<(String, Value)>,
}

impl<'a> Scope<'a> {
    pub(crate) fn new(parent: &'a dyn EvaluationContext) -> Self {
        Self {
            parent,
            locals: Vec::new(),
        }
    }

    /// Bind `name` to `value`, replacing an earlier local binding of the same name.
    pub(crate) fn set(&mut self, name: &str, value: Value) {
        match self.locals.iter_mut().find(|(n, _)| n == name) {
            Some(slot) => slot.1 = value,
            None => self.locals.push((name.to_string(), value)),
        }
    }

    /// Bind `name` to `NULL` and return its slot, for loops that swap items in and out with
    /// [`Scope::swap`] instead of cloning them.
    pub(crate) fn slot(&mut self, name: &str) -> usize {
        self.set(name, Value::Null);
        self.locals.iter().position(|(n, _)| n == name).unwrap_or(0)
    }

    /// Swap `value` with the value in `slot` (from [`Scope::slot`]).
    pub(crate) fn swap(&mut self, slot: usize, value: &mut Value) {
        if let Some((_, v)) = self.locals.get_mut(slot) {
            std::mem::swap(v, value);
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
        self.parent.get_custom_registry()
    }

    fn clone_variables(&self) -> HashMap<String, Value> {
        let mut vars = self.parent.clone_variables();
        for (n, v) in &self.locals {
            vars.insert(n.clone(), v.clone());
        }
        vars
    }
}

/// Evaluate a plain (non-lambda) method argument in the caller's context (variables and
/// custom functions).
pub(crate) fn eval_arg(expr: &Expr, ctx: &dyn EvaluationContext) -> Result<Value, Error> {
    Evaluator::eval(expr, &Scope::new(ctx))
}

fn missing_variable(name: &str) -> Error {
    Error::new(format!("Missing variable: :{}", name), None)
}

/// Excel-style date arithmetic, in days: `DateTime ± x` and `x + DateTime` give a DateTime
/// (fractional days allowed, rounded to the second), `DateTime - DateTime` gives the difference
/// in days (fractional). `x` converts like any operator operand (ADR-0027): NULL is 0, numeric
/// text and booleans convert, other text is `#VALUE!`. Anything else is the arithmetic type error.
#[cold]
#[inline(never)]
fn date_arithmetic(op: BinaryOp, a: &Value, b: &Value) -> Result<Value, Error> {
    const DAY: f64 = 86_400.0;
    let shift = |ts: i64, days: f64| {
        let secs = (days * DAY).round();
        // Stay inside i64 before the cast, then add with an overflow check.
        if secs.is_finite() && secs.abs() < 9.2e18 {
            ts.checked_add(secs as i64).map(Value::DateTime)
        } else {
            None
        }
        .ok_or_else(|| Error::new("Date arithmetic out of range", None))
    };
    let days = |v: &Value| coerced_operand(v, "Arithmetic op on non-number");
    match (op, a, b) {
        (BinaryOp::Sub, Value::DateTime(x), Value::DateTime(y)) => {
            Ok(Value::Number((*x as f64 - *y as f64) / DAY))
        }
        (_, Value::DateTime(_), Value::DateTime(_)) => {
            Err(Error::new("Arithmetic op on non-number", None))
        }
        (BinaryOp::Add, Value::DateTime(ts), x) | (BinaryOp::Add, x, Value::DateTime(ts)) => {
            shift(*ts, days(x)?)
        }
        (BinaryOp::Sub, Value::DateTime(ts), x) => shift(*ts, -days(x)?),
        _ => Err(Error::new("Arithmetic op on non-number", None)),
    }
}

/// An arithmetic operator with a Currency operand and no DateTime (ADR-0027). The other operand
/// converts like a number operand. `+ - * /` give Currency, except `Currency / Currency`, which
/// is a ratio (Number); `^` and `%` give Number. No rounding: Currency is a tagged `f64`.
#[cold]
#[inline(never)]
fn currency_arithmetic(op: BinaryOp, a: &Value, b: &Value) -> Result<Value, Error> {
    let operand = |v: &Value| match v {
        Value::Currency(c) => Ok(*c),
        _ => coerced_operand(v, "Arithmetic op on non-number"),
    };
    let (an, bn) = (operand(a)?, operand(b)?);
    let r = crate::runtime::arithmetic::checked_binary(op, an, bn)?;
    let ratio = matches!((a, b), (Value::Currency(_), Value::Currency(_)));
    Ok(match op {
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul => Value::Currency(r),
        BinaryOp::Div if !ratio => Value::Currency(r),
        _ => Value::Number(r),
    })
}

/// An arithmetic operator where an operand is not a number (BL-066). A DateTime operand is date
/// arithmetic and a Currency operand is currency arithmetic (ADR-0027). Otherwise each operand
/// converts as in Excel: numeric text, `TRUE`/`FALSE` as 1/0 and NULL (a blank cell) as 0; other
/// text is `#VALUE!`, and other types stay the arithmetic type error.
#[cold]
#[inline(never)]
fn coerced_arithmetic(op: BinaryOp, a: &Value, b: &Value) -> Result<Value, Error> {
    if matches!(a, Value::DateTime(_)) || matches!(b, Value::DateTime(_)) {
        return date_arithmetic(op, a, b);
    }
    if matches!(a, Value::Currency(_)) || matches!(b, Value::Currency(_)) {
        return currency_arithmetic(op, a, b);
    }
    let operand = |v: &Value| coerced_operand(v, "Arithmetic op on non-number");
    let (an, bn) = (operand(a)?, operand(b)?);
    crate::runtime::arithmetic::checked_binary(op, an, bn).map(Value::Number)
}

/// Unary plus on a non-number: Currency is kept as it is (ADR-0027); other operands convert as
/// in `coerced_operand`.
#[cold]
#[inline(never)]
fn coerced_plus(v: &Value) -> Result<Value, Error> {
    match v {
        Value::Currency(c) => Ok(Value::Currency(*c)),
        _ => coerced_operand(v, "Unary '+' on non-number").map(Value::Number),
    }
}

/// Unary minus on a non-number: Currency stays Currency (ADR-0027); other operands convert as
/// in `coerced_operand`. `0.0 - n`, not `-n`: `-NULL` and `-FALSE` are 0, not -0.
#[cold]
#[inline(never)]
fn coerced_negate(v: &Value) -> Result<Value, Error> {
    match v {
        Value::Currency(c) => Ok(Value::Currency(0.0 - *c)),
        _ => coerced_operand(v, "Unary '-' on non-number").map(|n| Value::Number(0.0 - n)),
    }
}

/// An operator operand as a number: numbers as they are, NULL as 0 (a blank cell), numeric text
/// and booleans as `coerce_to_number`; `type_error` for types that do not convert.
#[cold]
fn coerced_operand(v: &Value, type_error: &str) -> Result<f64, Error> {
    match v {
        Value::Number(n) => Ok(*n),
        Value::Null => Ok(0.0),
        _ => coerce_to_number(v, "")?.ok_or_else(|| Error::new(type_error, None)),
    }
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

            Expr::Binary(l, op @ (BinaryOp::And | BinaryOp::Or), r) => {
                Self::eval_short_circuit(*op, l, r, context)
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
    #[inline(always)]
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
            UnaryOp::Plus => match v {
                Value::Number(n) => Ok(Value::Number(*n)),
                _ => coerced_plus(v),
            },
            UnaryOp::Minus => match v {
                Value::Number(n) => Ok(Value::Number(-*n)),
                _ => coerced_negate(v),
            },
            // `!x` is `NOT(x)` with a tighter precedence (ADR-0029 amendment 1 A2).
            UnaryOp::Not => Ok(Value::Boolean(logical::not(v)?)),
        }
    }

    /// `a AND b`, `a OR b` (and `&&`, `||`): both sides are conditions (`logical::condition`),
    /// and `b` is evaluated only when `a` does not decide the result, so `FALSE AND 1/0` is
    /// FALSE (ADR-0029 amendment 1 A3). The result is always a boolean.
    #[inline(never)]
    fn eval_short_circuit<C: EvaluationContext>(
        op: BinaryOp,
        l: &Expr,
        r: &Expr,
        context: &C,
    ) -> Result<Value, Error> {
        let name = logical_op_name(op);
        let a = logical::condition(name, &*Self::eval_operand(l, context)?)?;
        limits::tick()?;
        match (op, a) {
            (BinaryOp::And, false) => Ok(Value::Boolean(false)),
            (BinaryOp::Or, true) => Ok(Value::Boolean(true)),
            _ => Ok(Value::Boolean(logical::condition(
                name,
                &*Self::eval_operand(r, context)?,
            )?)),
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
                let (Some(an), Some(bn)) = (a.as_number(), b.as_number()) else {
                    return coerced_arithmetic(op, a, b);
                };
                crate::runtime::arithmetic::checked_binary(op, an, bn).map(Value::Number)
            }
            BinaryOp::Eq
            | BinaryOp::Ne
            | BinaryOp::Lt
            | BinaryOp::Le
            | BinaryOp::Gt
            | BinaryOp::Ge => crate::runtime::comparison::compare(op, a, b),
            BinaryOp::And | BinaryOp::Or => {
                let name = logical_op_name(op);
                let ab = logical::condition(name, a)?;
                let bb = logical::condition(name, b)?;
                Ok(Value::Boolean(if op == BinaryOp::And {
                    ab && bb
                } else {
                    ab || bb
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
        name == crate::parser::CONCAT_OP
            || matches!(name, "MERGE" | "CONCAT" | "TEXTJOIN")
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
                let cond = logical::condition("Ternary", &Self::eval(&args[0], context)?)?;
                return if cond {
                    Self::eval(&args[1], context)
                } else {
                    Self::eval(&args[2], context)
                };
            }
            // `a & b` (D-10): both sides joined as text with `CONCAT` rules.
            crate::parser::CONCAT_OP => {
                let [a, b] = args else {
                    return Err(Error::new("'&' expects 2 operands", None));
                };
                let joined = [Self::eval(a, context)?, Self::eval(b, context)?];
                return exec_builtin_fast("CONCAT", &joined);
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
            "SUMIF" => Self::eval_conditional(Aggregate::Sum, args, context),
            "AVERAGEIF" | "AVGIF" => Self::eval_conditional(Aggregate::Average, args, context),
            "COUNTIF" => Self::eval_conditional(Aggregate::Count, args, context),
            "IF" | "IFS" if !args.iter().any(|a| matches!(a, Expr::Spread(_))) => {
                Self::eval_lazy_if(name, args, context)
            }
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

    /// `IF` / `IFS` evaluate a condition, then only the branch it picks (D-8, Excel). Same
    /// argument checks and conditions (`logical::condition`) as the eager versions in
    /// `logical.rs`, which still run calls with a spread argument.
    fn eval_lazy_if<C: EvaluationContext>(
        name: &str,
        args: &[Expr],
        context: &C,
    ) -> Result<Value, Error> {
        let truthy = |e: &Expr| Self::eval(e, context).and_then(|v| logical::condition(name, &v));
        if name == "IF" {
            logical::if_arity(args.len())?;
            return if truthy(&args[0])? {
                Self::eval(&args[1], context)
            } else {
                args.get(2)
                    .map_or(Ok(Value::Boolean(false)), |e| Self::eval(e, context))
            };
        }
        logical::ifs_arity(args.len())?;
        for pair in args.chunks(2) {
            if truthy(&pair[0])? {
                return Self::eval(&pair[1], context);
            }
        }
        Err(logical::ifs_no_match())
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
                let mut scope = Scope::new(context);
                for it in items {
                    charge_value(&it)?;
                    scope.set(&param_name, it.clone());
                    if logical::condition("FILTER", &Self::eval(lambda, &scope)?)? {
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
        if let Value::String(_) = arr_v {
            // Excel FIND(find_text, within_text, [start_num]) (excel-compat D-6).
            let mut ev_args = vec![arr_v];
            for a in &args[1..] {
                ev_args.push(Self::eval(a, context)?);
            }
            return crate::runtime::string::find_text(&ev_args);
        }
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
                let mut scope = Scope::new(context);
                for it in items {
                    charge_value(&it)?;
                    scope.set(&param_name, it.clone());
                    if logical::condition("FIND", &Self::eval(lambda, &scope)?)? {
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
                let mut scope = Scope::new(context);
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
                let mut scope = Scope::new(context);
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

    /// `COUNTIF`, `SUMIF`, `AVERAGEIF` (alias `AVGIF`): `(range, criteria, [values])`.
    /// A criterion that reads `:x` is a lambda over each item whose result is a condition
    /// (`logical::condition`), even when the caller also defines `:x` (BL-081). Otherwise it is
    /// evaluated once: a string or number uses Excel's rules (`runtime::criteria`), and any other
    /// value is evaluated per item like a lambda.
    fn eval_conditional<C: EvaluationContext>(
        kind: Aggregate,
        args: &[Expr],
        context: &C,
    ) -> Result<Value, Error> {
        let name = kind.name();
        let with_values = kind != Aggregate::Count;
        if args.len() < 2 || args.len() > if with_values { 3 } else { 2 } {
            let sig = if with_values {
                "(range, criteria, [values])"
            } else {
                "(range, criteria)"
            };
            return Err(Error::new(format!("{} expects {}", name, sig), None));
        }
        let range = range_items(Self::eval(&args[0], context)?);
        let values = match args.get(2) {
            Some(e) => Some(range_items(Self::eval(e, context)?)),
            None => None,
        };
        let criteria_expr = &args[1];
        if !reads_item(criteria_expr) {
            let criterion = Self::eval(criteria_expr, context)
                .ok()
                .and_then(|v| Criterion::from_value(&v));
            if let Some(criterion) = criterion {
                let mut row = 0usize;
                return aggregate(kind, &range, values.as_deref(), |item| {
                    limits::tick_item(row)?;
                    row += 1;
                    criterion.matches(item)
                });
            }
        }
        let mut range = range;
        let mut flags = Self::lambda_flags(name, criteria_expr, &mut range, context)?.into_iter();
        aggregate(kind, &range, values.as_deref(), |_| {
            Ok(flags.next().unwrap_or(false))
        })
    }

    /// A lambda criterion's result for each item, in order. Each item is moved into `:x` and
    /// back, not cloned (PERF-3).
    fn lambda_flags<C: EvaluationContext>(
        name: &str,
        lambda: &Expr,
        items: &mut [Value],
        context: &C,
    ) -> Result<Vec<bool>, Error> {
        let mut scope = Scope::new(context);
        let x = scope.slot("x");
        let mut flags = Vec::with_capacity(items.len());
        for item in items.iter_mut() {
            charge_value(item)?;
            scope.swap(x, item);
            let result = Self::eval(lambda, &scope)?;
            scope.swap(x, item);
            flags.push(logical::condition(name, &result)?);
        }
        Ok(flags)
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

/// The name of a logical operator in its condition error (ADR-0029 amendment 1 A2).
fn logical_op_name(op: BinaryOp) -> &'static str {
    if op == BinaryOp::And {
        "AND"
    } else {
        "OR"
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

/// Evaluate with support for assignments and sequences (no custom functions).
pub fn eval_with_assignments(expr: &Expr, vars: &HashMap<String, Value>) -> Result<Value, Error> {
    eval_with_assignments_in(expr, vars, None).map(|(v, _)| v)
}

/// Evaluate with support for assignments and sequences, returning both result and variable
/// context (no custom functions).
pub fn eval_with_assignments_and_context(
    expr: &Expr,
    vars: &HashMap<String, Value>,
) -> Result<(Value, HashMap<String, Value>), Error> {
    eval_with_assignments_in(expr, vars, None)
}

/// Like [`eval_with_assignments_and_context`], with the custom functions of `custom_registry`
/// visible in every step (ADR-0016).
pub fn eval_with_assignments_and_custom(
    expr: &Expr,
    vars: &HashMap<String, Value>,
    custom_registry: &Arc<RwLock<FunctionRegistry>>,
) -> Result<(Value, HashMap<String, Value>), Error> {
    eval_with_assignments_in(expr, vars, Some(custom_registry))
}

fn eval_with_assignments_in(
    expr: &Expr,
    vars: &HashMap<String, Value>,
    custom_registry: Option<&Arc<RwLock<FunctionRegistry>>>,
) -> Result<(Value, HashMap<String, Value>), Error> {
    limits::evaluation(|| {
        let mut context = VariableContext {
            variables: Cow::Owned(vars.clone()),
            custom_registry,
        };
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
