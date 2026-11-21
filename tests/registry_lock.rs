//! A running custom function does not hold the global function registry lock: registering
//! (as `/reload-hooks` does) and nested custom calls proceed while it runs.
//! Own test binary because it uses the process-global registry.

use skillet::{evaluate_with_custom, register_function, CustomFunction, Error, Value};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::time::Duration;

/// Signals `started`, then blocks until `release` receives (or 30 s pass).
struct Blocking {
    started: Mutex<Sender<()>>,
    release: Mutex<Receiver<()>>,
}

impl CustomFunction for Blocking {
    fn name(&self) -> &str {
        "LOCKTEST_BLOCKING"
    }
    fn min_args(&self) -> usize {
        0
    }
    fn max_args(&self) -> Option<usize> {
        Some(0)
    }
    fn execute(&self, _args: Vec<Value>) -> Result<Value, Error> {
        let _ = self.started.lock().unwrap().send(());
        let _ = self
            .release
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(30));
        Ok(Value::Number(1.0))
    }
}

struct Constant(&'static str);

impl CustomFunction for Constant {
    fn name(&self) -> &str {
        self.0
    }
    fn min_args(&self) -> usize {
        0
    }
    fn max_args(&self) -> Option<usize> {
        Some(1)
    }
    fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
        Ok(args.into_iter().next().unwrap_or(Value::Number(2.0)))
    }
}

#[test]
fn registering_does_not_wait_for_a_running_custom_function() {
    let (started_tx, started_rx) = channel();
    let (release_tx, release_rx) = channel();
    register_function(Box::new(Blocking {
        started: Mutex::new(started_tx),
        release: Mutex::new(release_rx),
    }))
    .unwrap();

    let eval = std::thread::spawn(|| evaluate_with_custom("=LOCKTEST_BLOCKING()", &HashMap::new()));
    started_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("custom function started");

    let (registered_tx, registered_rx) = channel();
    std::thread::spawn(move || {
        let r = register_function(Box::new(Constant("LOCKTEST_LATE")));
        let _ = registered_tx.send(r.is_ok());
    });
    let registered = registered_rx.recv_timeout(Duration::from_secs(5));

    let _ = release_tx.send(());
    assert_eq!(eval.join().unwrap().unwrap(), Value::Number(1.0));
    assert_eq!(
        registered,
        Ok(true),
        "register_function waited for the running custom function"
    );
}

#[test]
fn nested_custom_calls_still_work() {
    register_function(Box::new(Constant("LOCKTEST_OUTER"))).unwrap();
    register_function(Box::new(Constant("LOCKTEST_INNER"))).unwrap();
    let v = evaluate_with_custom("=LOCKTEST_OUTER(LOCKTEST_INNER() + 1)", &HashMap::new());
    assert_eq!(v.unwrap(), Value::Number(3.0));
    let err = evaluate_with_custom("=LOCKTEST_INNER(1, 2)", &HashMap::new()).unwrap_err();
    assert!(err.message.contains("at most 1"), "{}", err.message);
}
