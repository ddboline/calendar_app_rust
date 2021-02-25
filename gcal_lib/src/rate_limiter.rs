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
        let current = Utc::now().timestamp_millis() as usize;
        let until = self.until.load(Ordering::SeqCst);
        if current > until {
            let new = current + self.unit_time_ms;
            if self
                .until
                .compare_exchange(until, new, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                self.current_count
                    .store(self.max_per_unit_time as isize, Ordering::SeqCst);
            }
        }
    }

    pub async fn acquire(&self) {
        loop {
            self.check_reset();

            if self.current_count.fetch_sub(1, Ordering::SeqCst) > 0 {
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
                if self.current_count.fetch_sub(1, Ordering::SeqCst) > 0 {
                    return;
                }
            } else {
                let _lock = self.limit_mutex.lock().await;
                self.check_reset();
                if self.current_count.fetch_sub(1, Ordering::SeqCst) > 0 {
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Error;
    use chrono::Utc;
    use log::debug;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tokio::{
        task::spawn,
        time::{sleep, Duration},
    };

    use crate::rate_limiter::RateLimiter;

    #[tokio::test]
    async fn test_rate_limiter() -> Result<(), Error> {
        env_logger::init();

        let rate_limiter = Arc::new(RateLimiter::new(1000, 100));
        let test_count = Arc::new(AtomicUsize::new(0));

        let start = Utc::now();

        let tasks: Vec<_> = (0..10_000)
            .map(|_| {
                let rate_limiter = rate_limiter.clone();
                let test_count = test_count.clone();
                spawn(async move {
                    rate_limiter.acquire().await;
                    test_count.fetch_add(1, Ordering::SeqCst);
                })
            })
            .collect();

        sleep(Duration::from_millis(20)).await;

        let count = test_count.load(Ordering::SeqCst);
        debug!("{}", count);
        assert_eq!(count,  1000);

        sleep(Duration::from_millis(100)).await;

        let count = test_count.load(Ordering::SeqCst);
        assert_eq!(count, 2000);

        for t in tasks {
            t.await?;
        }

        let elapsed = Utc::now() - start;

        debug!("{}", elapsed);
        assert!(elapsed.num_seconds() >= 1);
        assert_eq!(test_count.load(Ordering::SeqCst), 10_000);
        Ok(())
    }
}
