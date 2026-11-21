use crate::custom::CustomFunction;
use crate::error::Error;
use crate::limits;
use crate::types::Value;
use crate::MAX_NESTING_DEPTH;
use rquickjs::convert::Coerced;
use rquickjs::{Ctx, FromJs, Function as JsFunction, IntoJs, Runtime};
use std::cell::Cell;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

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
                min_args = rest
                    .trim()
                    .parse()
                    .map_err(|_| Error::new("Invalid min_args value", None))?;
            } else if let Some(rest) = line.strip_prefix("// @max_args:") {
                if rest.trim() == "unlimited" {
                    max_args = None;
                } else {
                    max_args = Some(
                        rest.trim()
                            .parse()
                            .map_err(|_| Error::new("Invalid max_args value", None))?,
                    );
                }
            } else if let Some(rest) = line.strip_prefix("// @description:") {
                description = Some(rest.trim().to_string());
            } else if let Some(rest) = line.strip_prefix("// @example:") {
                example = Some(rest.trim().to_string());
            }
        }

        let name =
            name.ok_or_else(|| Error::new("JavaScript function must have @name annotation", None))?;

        Self::new(
            name,
            min_args,
            max_args,
            description,
            example,
            js_code.to_string(),
        )
    }

    /// Convert a Skillet value into a JS value. `depth` is the number of arrays enclosing
    /// `value`; arrays nest at most [`MAX_NESTING_DEPTH`] levels (an empty array adds none).
    fn value_to_js<'js>(
        ctx: &Ctx<'js>,
        value: &Value,
        depth: usize,
    ) -> Result<rquickjs::Value<'js>, Failure> {
        let js = match value {
            Value::Number(n) => n.into_js(ctx)?,
            Value::String(s) | Value::Json(s) => s.as_str().into_js(ctx)?,
            Value::Boolean(b) => b.into_js(ctx)?,
            Value::Null => ().into_js(ctx)?,
            Value::Currency(c) => c.into_js(ctx)?,
            Value::DateTime(dt) => (*dt as f64).into_js(ctx)?,
            Value::Array(arr) => {
                if !arr.is_empty() && depth >= MAX_NESTING_DEPTH {
                    return Err(Failure::Value(limits::nesting_error(None)));
                }
                let js_array = rquickjs::Array::new(ctx.clone())?;
                for (i, item) in arr.iter().enumerate() {
                    js_array.set(i, Self::value_to_js(ctx, item, depth + 1)?)?;
                }
                js_array.into_value()
            }
        };
        Ok(js)
    }

    /// Convert a JS value into a Skillet value.
    ///
    /// `depth` is the number of arrays enclosing `js_val`. Arrays nest at most
    /// [`MAX_NESTING_DEPTH`] levels (counted like the evaluator: an empty array adds no level),
    /// which also stops self-referencing arrays before the recursion exhausts the stack.
    /// Objects go through `JSON.stringify`, which throws on cycles.
    fn js_to_value<'js>(
        ctx: &Ctx<'js>,
        js_val: rquickjs::Value<'js>,
        depth: usize,
    ) -> Result<Value, Failure> {
        if js_val.is_null() || js_val.is_undefined() {
            Ok(Value::Null)
        } else if js_val.is_bool() {
            Ok(Value::Boolean(FromJs::from_js(ctx, js_val)?))
        } else if js_val.is_number() {
            Ok(Value::Number(FromJs::from_js(ctx, js_val)?))
        } else if js_val.is_string() {
            Ok(Value::String(FromJs::from_js(ctx, js_val)?))
        } else if js_val.is_array() {
            let js_array: rquickjs::Array = FromJs::from_js(ctx, js_val)?;
            let length = js_array.len();
            if length > 0 && depth >= MAX_NESTING_DEPTH {
                return Err(Failure::Value(limits::nesting_error(None)));
            }
            let mut result = Vec::with_capacity(length.min(1024));
            for i in 0..length {
                let item = js_array.get::<rquickjs::Value>(i)?;
                result.push(Self::js_to_value(ctx, item, depth + 1)?);
            }
            Ok(Value::Array(result))
        } else if js_val.is_object() {
            let json: rquickjs::Object = ctx.globals().get("JSON")?;
            let stringify: JsFunction = json.get("stringify")?;
            let json_result: rquickjs::Value = stringify.call((js_val,))?;
            if json_result.is_string() {
                Ok(Value::Json(FromJs::from_js(ctx, json_result)?))
            } else {
                // Fallback for non-serializable objects
                Ok(Value::String("[object Object]".to_string()))
            }
        } else {
            // For other types, convert to string representation
            let s: String =
                FromJs::from_js(ctx, js_val).unwrap_or_else(|_| "[unknown type]".to_string());
            Ok(Value::String(s))
        }
    }

    /// Evaluate the hook source, call `execute(args)` and convert the result.
    fn run<'js>(&self, ctx: &Ctx<'js>, args: &[Value]) -> Result<Value, Failure> {
        ctx.eval::<(), _>(self.js_code.as_bytes())?;

        let execute_fn: JsFunction = ctx.globals().get("execute").map_err(|_| {
            Failure::Value(Error::new(
                format!("JS function {} has no function 'execute'", self.name),
                None,
            ))
        })?;

        let js_args = rquickjs::Array::new(ctx.clone())?;
        for (i, arg) in args.iter().enumerate() {
            js_args.set(i, Self::value_to_js(ctx, arg, 0)?)?;
        }

        let result: rquickjs::Value = execute_fn.call((js_args,))?;
        Self::js_to_value(ctx, result, 0)
    }

    /// The error for a failed call, per ADR-0017 §5.
    fn failure_error(&self, failed: Failed, stop: Option<Stop>) -> Error {
        match stop {
            Some(Stop::Deadline) => return limits::trip_deadline(),
            Some(Stop::CallCap) => {
                return Error::new(
                    format!(
                        "JS function {} timed out (max {})",
                        self.name,
                        limits::format_duration(call_timeout())
                    ),
                    None,
                )
            }
            None => {}
        }
        let text = match failed {
            Failed::Final(e) => return e,
            Failed::Js(text) => text,
        };
        if text.ends_with(OUT_OF_MEMORY) || text == THROWN_NULL {
            Error::new(
                format!(
                    "JS function {} ran out of memory (max {} MB)",
                    self.name,
                    JS_MEMORY_LIMIT / MIB
                ),
                None,
            )
        } else {
            Error::new(format!("JS function {} failed: {}", self.name, text), None)
        }
    }
}

/// Per-call cap on the QuickJS heap (ADR-0017 §1). Only enforced with QuickJS's own allocator:
/// never enable rquickjs's `rust-alloc`/`allocator` features (see `Cargo.toml`).
const JS_MEMORY_LIMIT: usize = 64 * MIB;
/// QuickJS stack limit (ADR-0017 §2), the rquickjs default, set explicitly. Must stay well below
/// the smallest thread that runs JS (2 MB for a default `std::thread`).
const JS_MAX_STACK: usize = 256 * 1024;
/// Per-call wall-clock cap (ADR-0017 §3), on top of the evaluation deadline when there is one.
const JS_CALL_TIMEOUT: Duration = Duration::from_secs(10);
const MIB: usize = 1024 * 1024;
/// QuickJS's message when an allocation fails (the memory limit).
const OUT_OF_MEMORY: &str = "out of memory";
/// What [`exception_text`] returns for a thrown `null`. QuickJS throws `null` when the memory
/// limit leaves no room even for its `InternalError: out of memory` object, so it is reported as
/// running out of memory (a hook that runs `throw null` itself gets the same text).
const THROWN_NULL: &str = "null";

#[cfg(test)]
thread_local! {
    /// Test-only override of [`JS_CALL_TIMEOUT`] for calls on this thread, to keep tests fast.
    static CALL_TIMEOUT_OVERRIDE: std::cell::Cell<Option<Duration>> =
        const { std::cell::Cell::new(None) };
}

fn call_timeout() -> Duration {
    #[cfg(test)]
    if let Some(d) = CALL_TIMEOUT_OVERRIDE.with(|c| c.get()) {
        return d;
    }
    JS_CALL_TIMEOUT
}

/// Why the interrupt handler stopped a call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stop {
    /// The active evaluation's deadline passed (ADR-0011).
    Deadline,
    /// [`JS_CALL_TIMEOUT`] passed since the call started.
    CallCap,
}

/// A failed step inside a call: a QuickJS error (exception pending in the context) or a
/// Skillet error that is already final (nesting depth, missing `execute`).
enum Failure {
    Js(rquickjs::Error),
    Value(Error),
}

impl From<rquickjs::Error> for Failure {
    fn from(e: rquickjs::Error) -> Self {
        Failure::Js(e)
    }
}

/// A [`Failure`] read out of the context (the exception text taken with `ctx.catch()`), so it can
/// be reported after the context is released.
enum Failed {
    Final(Error),
    Js(String),
}

impl Failed {
    fn read(ctx: &Ctx<'_>, failure: Failure) -> Failed {
        match failure {
            Failure::Value(e) => Failed::Final(e),
            Failure::Js(rquickjs::Error::Exception) => Failed::Js(exception_text(ctx)),
            Failure::Js(rquickjs::Error::Allocation) => Failed::Js(OUT_OF_MEMORY.to_string()),
            Failure::Js(e) => Failed::Js(e.to_string()),
        }
    }
}

/// Where the interrupt handler records why it stopped a call (shared with `execute`).
type StopCell = Rc<Cell<Option<Stop>>>;

/// A fresh runtime with the ADR-0017 limits, and the cell its interrupt handler records a stop
/// in. The handler only reads the clock; QuickJS calls it regularly while bytecode runs, and the
/// exception it raises cannot be caught by the hook.
fn limited_runtime() -> Result<(Runtime, StopCell), Error> {
    let runtime = Runtime::new()
        .map_err(|e| Error::new(format!("Failed to create JS runtime: {}", e), None))?;
    runtime.set_memory_limit(JS_MEMORY_LIMIT);
    runtime.set_max_stack_size(JS_MAX_STACK);

    let stop = Rc::new(Cell::new(None));
    let seen = Rc::clone(&stop);
    let started = Instant::now();
    let cap = call_timeout();
    runtime.set_interrupt_handler(Some(Box::new(move || {
        if seen.get().is_none() {
            if limits::deadline_passed() {
                seen.set(Some(Stop::Deadline));
            } else if started.elapsed() >= cap {
                seen.set(Some(Stop::CallCap));
            }
        }
        seen.get().is_some()
    })));
    Ok((runtime, stop))
}

/// The pending exception as `Name: message` (`RangeError: Maximum call stack size exceeded`),
/// or the thrown value as text when it is not an `Error` (`throw "boom"`).
fn exception_text(ctx: &Ctx<'_>) -> String {
    let thrown = ctx.catch();
    if let Some(exception) = thrown.as_exception() {
        let name = exception
            .get::<_, Option<Coerced<String>>>("name")
            .ok()
            .flatten()
            .map(|c| c.0)
            .unwrap_or_default();
        let message = exception.message().unwrap_or_default();
        match (name.is_empty(), message.is_empty()) {
            (false, false) => format!("{}: {}", name, message),
            (false, true) => name,
            _ => message,
        }
    } else {
        thrown
            .get::<Coerced<String>>()
            .map(|c| c.0)
            .unwrap_or_else(|_| "unknown exception".to_string())
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

    /// Runs the hook in a fresh QuickJS runtime limited to 64 MiB of heap, a 256 KiB stack and
    /// 10 s (or the evaluation deadline, whichever comes first). Hooks get no host functions.
    fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
        let (runtime, stop) = limited_runtime()?;
        let ctx = rquickjs::Context::full(&runtime)
            .map_err(|e| Error::new(format!("Failed to create JS context: {}", e), None))?;
        let outcome = ctx.with(|ctx| {
            self.run(&ctx, &args)
                .map_err(|failure| Failed::read(&ctx, failure))
        });
        outcome.map_err(|failed| self.failure_error(failed, stop.get()))
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
            fs::create_dir_all(hooks_path).map_err(|e| {
                Error::new(format!("Failed to create hooks directory: {}", e), None)
            })?;
            return Ok(Vec::new());
        }

        let root = hooks_path
            .canonicalize()
            .map_err(|e| Error::new(format!("Failed to read hooks directory: {}", e), None))?;
        let mut visited = HashSet::from([root.clone()]);
        let mut functions = Vec::new();
        Self::load_functions_recursive(&root, &root, &mut visited, &mut functions)?;
        Ok(functions)
    }

    /// Recursively load JavaScript functions from a directory. Entries are resolved first:
    /// anything that resolves outside `root` is skipped, and each directory is read once.
    fn load_functions_recursive(
        dir: &Path,
        root: &Path,
        visited: &mut HashSet<PathBuf>,
        functions: &mut Vec<Box<dyn CustomFunction>>,
    ) -> Result<(), Error> {
        let entries = fs::read_dir(dir)
            .map_err(|e| Error::new(format!("Failed to read directory: {}", e), None))?;

        for entry in entries {
            let entry = entry
                .map_err(|e| Error::new(format!("Failed to read directory entry: {}", e), None))?;

            let path = entry.path();
            let real = match path.canonicalize() {
                Ok(real) if real.starts_with(root) => real,
                Ok(_) => {
                    eprintln!("Warning: Skipping {:?}: outside the hooks directory", path);
                    continue;
                }
                Err(e) => {
                    eprintln!("Warning: Skipping {:?}: {}", path, e);
                    continue;
                }
            };

            if real.is_dir() {
                if visited.insert(real.clone()) {
                    Self::load_functions_recursive(&real, root, visited, functions)?;
                }
            } else if path.extension().and_then(|s| s.to_str()) == Some("js") {
                match JavaScriptFunction::from_file(&real) {
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
        let result = js_func
            .execute(vec![Value::String("hello".to_string())])
            .unwrap();

        match result {
            Value::String(s) => assert_eq!(s, "olleh"),
            _ => panic!("Expected string result"),
        }
    }

    fn hook_returning(body: &str) -> JavaScriptFunction {
        let js_code = format!(
            "// @name: NESTED\n// @min_args: 0\nfunction execute(args) {{ {} }}",
            body
        );
        JavaScriptFunction::parse_js_function_internal(&js_code).unwrap()
    }

    fn nested_array_js(depth: usize) -> String {
        format!(
            "var a = [1]; for (var i = 1; i < {}; i++) {{ a = [a]; }} return a;",
            depth
        )
    }

    fn array_depth(v: &Value) -> usize {
        match v {
            Value::Array(items) => 1 + items.iter().map(array_depth).max().unwrap_or(0),
            _ => 0,
        }
    }

    #[test]
    fn hook_result_at_max_nesting_depth_converts() {
        let result = hook_returning(&nested_array_js(MAX_NESTING_DEPTH))
            .execute(vec![])
            .unwrap();
        assert_eq!(array_depth(&result), MAX_NESTING_DEPTH);
    }

    #[test]
    fn hook_result_past_max_nesting_depth_is_rejected() {
        let err = hook_returning(&nested_array_js(MAX_NESTING_DEPTH + 1))
            .execute(vec![])
            .unwrap_err();
        assert_eq!(err.message, "Expression nested too deeply (max 128)");
    }

    #[test]
    fn hook_result_deeply_nested_array_is_rejected() {
        let err = hook_returning(&nested_array_js(200_000))
            .execute(vec![])
            .unwrap_err();
        assert_eq!(err.message, "Expression nested too deeply (max 128)");
    }

    #[test]
    fn hook_result_self_referencing_array_is_rejected() {
        let err = hook_returning("var a = [1]; a.push(a); return a;")
            .execute(vec![])
            .unwrap_err();
        assert_eq!(err.message, "Expression nested too deeply (max 128)");
    }

    #[test]
    fn hook_result_empty_array_below_max_depth_converts() {
        // An empty array adds no nesting level, as in the evaluator's depth rule.
        let body = format!(
            "var a = []; for (var i = 0; i < {}; i++) {{ a = [a]; }} return a;",
            MAX_NESTING_DEPTH
        );
        assert!(hook_returning(&body).execute(vec![]).is_ok());
    }

    /// Run `f` with the per-call cap shortened to `cap` on this thread.
    fn with_call_cap<R>(cap: Duration, f: impl FnOnce() -> R) -> R {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                CALL_TIMEOUT_OVERRIDE.with(|c| c.set(None));
            }
        }
        CALL_TIMEOUT_OVERRIDE.with(|c| c.set(Some(cap)));
        let _reset = Reset;
        f()
    }

    fn error_of(body: &str) -> String {
        hook_returning(body).execute(vec![]).unwrap_err().message
    }

    #[test]
    fn looping_hook_is_stopped_by_the_call_cap() {
        let started = Instant::now();
        let err = with_call_cap(Duration::from_millis(100), || error_of("while (true) {}"));
        assert_eq!(err, "JS function NESTED timed out (max 100 ms)");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "{:?}",
            started.elapsed()
        );
    }

    #[test]
    fn loop_inside_try_catch_is_still_interrupted() {
        let body = "for (;;) { try { while (true) {} } catch (e) {} }";
        let err = with_call_cap(Duration::from_millis(100), || error_of(body));
        assert_eq!(err, "JS function NESTED timed out (max 100 ms)");
    }

    #[test]
    fn looping_hook_is_stopped_by_the_evaluation_deadline() {
        let limits = crate::EvalLimits {
            timeout: Some(Duration::from_millis(100)),
            ..Default::default()
        };
        let started = Instant::now();
        crate::with_limits(limits, || {
            let err = error_of("while (true) {}");
            assert_eq!(err, "Evaluation timed out (max 100 ms)");
            // The trip is recorded, so the entry point reports it even if swallowed.
            let swallowed = crate::limits::evaluation(|| Ok(1));
            assert_eq!(swallowed.unwrap_err().message, err);
        });
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "{:?}",
            started.elapsed()
        );
    }

    #[test]
    fn hook_that_allocates_400_mb_runs_out_of_memory() {
        let err = error_of("return 'x'.repeat(4e8).length;");
        assert_eq!(err, "JS function NESTED ran out of memory (max 64 MB)");
        let err = error_of("var a = []; for (;;) a.push({ k: [1, 2, 3] });");
        assert_eq!(err, "JS function NESTED ran out of memory (max 64 MB)");
    }

    #[test]
    fn runaway_recursion_reports_the_range_error() {
        let err = error_of("function f(n) { return f(n + 1) + 1; } return f(0);");
        assert_eq!(
            err,
            "JS function NESTED failed: RangeError: Maximum call stack size exceeded"
        );
    }

    #[test]
    fn other_exceptions_report_their_message() {
        assert_eq!(
            error_of("throw new TypeError('bad input');"),
            "JS function NESTED failed: TypeError: bad input"
        );
        assert_eq!(error_of("throw 'boom';"), "JS function NESTED failed: boom");
    }

    #[test]
    fn hooks_have_no_http_get() {
        let result = hook_returning("return typeof httpGet;")
            .execute(vec![])
            .unwrap();
        assert_eq!(result, Value::String("undefined".to_string()));
    }

    #[test]
    fn deep_argument_is_rejected_not_overflowed() {
        let nest = |depth: usize| {
            let mut v = Value::Number(1.0);
            for _ in 0..depth {
                v = Value::Array(vec![v]);
            }
            v
        };
        let hook = hook_returning("return 1;");
        assert!(hook.execute(vec![nest(MAX_NESTING_DEPTH)]).is_ok());
        let err = hook.execute(vec![nest(MAX_NESTING_DEPTH + 1)]).unwrap_err();
        assert_eq!(err.message, "Expression nested too deeply (max 128)");
    }

    #[cfg(unix)]
    #[test]
    fn loader_skips_links_that_leave_the_hooks_dir() {
        let hook =
            |name: &str| format!("// @name: {}\nfunction execute(args) {{ return 1; }}", name);
        let base = tempfile::tempdir().unwrap();
        let hooks = base.path().join("hooks");
        let elsewhere = base.path().join("elsewhere");
        fs::create_dir_all(hooks.join("sub")).unwrap();
        fs::create_dir(&elsewhere).unwrap();
        fs::write(hooks.join("inside.js"), hook("INSIDE")).unwrap();
        fs::write(hooks.join("sub").join("nested.js"), hook("NESTED")).unwrap();
        fs::write(elsewhere.join("far.js"), hook("FAR")).unwrap();
        fs::write(base.path().join("outside.js"), hook("OUTSIDE")).unwrap();
        std::os::unix::fs::symlink(&elsewhere, hooks.join("linked_dir")).unwrap();
        std::os::unix::fs::symlink(base.path().join("outside.js"), hooks.join("linked.js"))
            .unwrap();
        std::os::unix::fs::symlink(&hooks, hooks.join("sub").join("loop")).unwrap();

        let loader = JSPluginLoader::new(hooks.to_str().unwrap().to_string());
        let mut names: Vec<String> = loader
            .load_functions()
            .unwrap()
            .iter()
            .map(|f| f.name().to_string())
            .collect();
        names.sort();
        assert_eq!(names, ["INSIDE", "NESTED"]);
    }
}
