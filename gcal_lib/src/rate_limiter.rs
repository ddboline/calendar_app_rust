use tokio::sync::{Semaphore, SemaphorePermit, TryAcquireError};
use std::sync::atomic::{AtomicUsize, AtomicIsize, Ordering};
use anyhow::Error;
use chrono::Utc;
use tokio::task::JoinHandle;
use tokio::time::{Duration, sleep};

pub struct RateLimiter {
    max_per_unit_time: usize,
    unit_time_ms: usize,
    until: AtomicUsize,
    current_count: AtomicIsize,
    number_of_waits: AtomicUsize,
    total_wait_ms: AtomicUsize,
}

impl RateLimiter {
    pub fn new(max_per_unit_time: usize, unit_time_ms: usize) -> Self {
        Self {
            max_per_unit_time,
            unit_time_ms,
            until: AtomicUsize::new(Utc::now().timestamp_millis() as usize),
            current_count: AtomicIsize::new(max_per_unit_time as isize),
            number_of_waits: AtomicUsize::new(0),
            total_wait_ms: AtomicUsize::new(0),
        }
    }

    pub fn get_number_of_waits(&self) -> usize {
        self.number_of_waits.load(Ordering::SeqCst)
    }

    pub fn get_total_wait_ms(&self) -> usize {
        self.total_wait_ms.load(Ordering::SeqCst)
    }

    pub async fn acquire(&self) {
        loop {
            if Utc::now().timestamp_millis() as usize > self.until.load(Ordering::SeqCst) {
                let new = (Utc::now() + chrono::Duration::milliseconds(self.unit_time_ms as i64)).timestamp_millis() as usize;
                self.until.store(new, Ordering::SeqCst);
                self.current_count.store(self.max_per_unit_time as isize, Ordering::SeqCst);
            }
            if self.current_count.fetch_sub(1, Ordering::SeqCst) > 1 {
                return;
            } else {
                self.current_count.fetch_add(1, Ordering::SeqCst);
                let remaining = self.until.load(Ordering::SeqCst) as i64 - Utc::now().timestamp_millis();
                if remaining > 0 {
                    self.number_of_waits.fetch_add(1, Ordering::SeqCst);
                    sleep(Duration::from_millis(remaining as u64)).await;
                    self.total_wait_ms.fetch_add(remaining as usize, Ordering::SeqCst);
                }
                if self.current_count.fetch_sub(1, Ordering::SeqCst) > 1 {
                    return;
                }
            }
        }
    }
}