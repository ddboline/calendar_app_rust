use anyhow::Error;
use chrono::Utc;
use std::sync::atomic::{AtomicIsize, AtomicUsize, Ordering};
use tokio::{
    sync::Mutex,
    task::JoinHandle,
    time::{sleep, Duration},
};

pub struct RateLimiter {
    max_per_unit_time: usize,
    unit_time_ms: usize,
    until: AtomicUsize,
    current_count: AtomicIsize,
    limit_mutex: Mutex<()>,
}

impl RateLimiter {
    pub fn new(max_per_unit_time: usize, unit_time_ms: usize) -> Self {
        Self {
            max_per_unit_time,
            unit_time_ms,
            until: AtomicUsize::new(Utc::now().timestamp_millis() as usize),
            current_count: AtomicIsize::new(max_per_unit_time as isize),
            limit_mutex: Mutex::new(()),
        }
    }

    fn check_reset(&self) {
        if Utc::now().timestamp_millis() as usize > self.until.load(Ordering::SeqCst) {
            let new = (Utc::now() + chrono::Duration::milliseconds(self.unit_time_ms as i64))
                .timestamp_millis() as usize;
            self.until.store(new, Ordering::SeqCst);
            self.current_count
                .store(self.max_per_unit_time as isize, Ordering::SeqCst);
        }
    }

    pub async fn acquire(&self) {
        loop {
            self.check_reset();

            if self.current_count.fetch_sub(1, Ordering::SeqCst) > 1 {
                return;
            }

            if let Ok(_lock) = self.limit_mutex.try_lock() {
                self.current_count.fetch_add(1, Ordering::SeqCst);
                let current = Utc::now().timestamp_millis();
                let remaining = self.until.load(Ordering::SeqCst) as i64 - current;
                if remaining > 0 {
                    sleep(Duration::from_millis(remaining as u64)).await;
                }
                self.check_reset();
                if self.current_count.fetch_sub(1, Ordering::SeqCst) > 1 {
                    return;
                }
            } else {
                let _lock = self.limit_mutex.lock().await;
                self.check_reset();
                if self.current_count.fetch_sub(1, Ordering::SeqCst) > 1 {
                    return;
                }
            }
        }
    }
}
