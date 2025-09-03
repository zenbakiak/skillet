use std::sync::atomic::{AtomicU64, Ordering};

pub struct ServerStats {
    requests_processed: AtomicU64,
    total_execution_time: AtomicU64, // in microseconds
}

impl ServerStats {
    pub fn new() -> Self {
        Self {
            requests_processed: AtomicU64::new(0),
            total_execution_time: AtomicU64::new(0),
        }
    }

    pub fn record_request(&self, execution_time_us: u64) {
        self.requests_processed.fetch_add(1, Ordering::Relaxed);
        self.total_execution_time.fetch_add(execution_time_us, Ordering::Relaxed);
    }

    pub fn get_stats(&self) -> (u64, f64) {
        let count = self.requests_processed.load(Ordering::Relaxed);
        let total_time = self.total_execution_time.load(Ordering::Relaxed);
        let avg_time_ms = if count > 0 {
            total_time as f64 / count as f64 / 1000.0
        } else { 0.0 };
        (count, avg_time_ms)
    }
}