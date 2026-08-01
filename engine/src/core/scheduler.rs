use serde::Serialize;
use std::collections::VecDeque;
use std::future::Future;
use std::sync::{Arc, Mutex};
use tokio::sync::Semaphore;

/// A generic task scheduler that runs queued jobs with a bounded concurrency
/// level and records execution stats. Used by the engine to sequence module
/// runs while keeping the pipeline observable.
#[derive(Default, Serialize, Clone)]
pub struct SchedulerStats {
    pub queued: u64,
    pub running: u64,
    pub completed: u64,
    pub failed: u64,
}

#[derive(Clone)]
pub struct Scheduler {
    concurrency: usize,
    semaphore: Arc<Semaphore>,
    stats: Arc<Mutex<SchedulerStats>>,
}

impl Scheduler {
    pub fn new(concurrency: usize) -> Self {
        Scheduler {
            concurrency: concurrency.max(1),
            semaphore: Arc::new(Semaphore::new(concurrency.max(1))),
            stats: Arc::new(Mutex::new(SchedulerStats::default())),
        }
    }

    pub fn queue_len(&self) -> usize {
        self.semaphore.available_permits()
    }

    pub fn stats(&self) -> SchedulerStats {
        self.stats.lock().unwrap().clone()
    }

    /// Enqueue an async job. Returns immediately; the job runs when a permit
    /// is available. `done` is a callback executed after the job finishes.
    pub fn enqueue<F, D>(&self, job: F, done: D)
    where
        F: Future<Output = Result<(), String>> + Send + 'static,
        D: FnOnce(bool) + Send + 'static,
    {
        let sem = self.semaphore.clone();
        let stats = self.stats.clone();
        {
            let mut s = stats.lock().unwrap();
            s.queued += 1;
        }
        tokio::spawn(async move {
            let _permit = sem.acquire_owned().await.unwrap();
            {
                let mut s = stats.lock().unwrap();
                s.running += 1;
            }
            let ok = match job.await {
                Ok(_) => {
                    let mut s = stats.lock().unwrap();
                    s.completed += 1;
                    true
                }
                Err(_) => {
                    let mut s = stats.lock().unwrap();
                    s.failed += 1;
                    false
                }
            };
            {
                let mut s = stats.lock().unwrap();
                s.running = s.running.saturating_sub(1);
            }
            done(ok);
        });
    }
}

/// Pending jobs waiting for their turn, exposed for observability.
pub struct JobQueue<T> {
    inner: Mutex<VecDeque<T>>,
}

impl<T> Default for JobQueue<T> {
    fn default() -> Self {
        JobQueue {
            inner: Mutex::new(VecDeque::new()),
        }
    }
}

impl<T> JobQueue<T> {
    pub fn push(&self, item: T) {
        self.inner.lock().unwrap().push_back(item);
    }

    pub fn pop(&self) -> Option<T> {
        self.inner.lock().unwrap().pop_front()
    }

    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
