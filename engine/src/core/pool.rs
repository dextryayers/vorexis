use futures::future::join_all;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::Semaphore;

pub struct WorkerPool {
    semaphore: Arc<Semaphore>,
}

impl WorkerPool {
    pub fn new(concurrency: usize) -> Self {
        WorkerPool {
            semaphore: Arc::new(Semaphore::new(concurrency.max(1))),
        }
    }

    /// Run `n` closures concurrently, bounded by the pool's concurrency limit.
    pub async fn run<T, O, F, Fut>(&self, items: Vec<T>, f: F) -> Vec<O>
    where
        T: Send + 'static,
        O: Send + 'static,
        F: Fn(T) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = O> + Send + 'static,
    {
        let f = Arc::new(f);
        let handles: Vec<_> = items
            .into_iter()
            .map(|item| {
                let sem = self.semaphore.clone();
                let f = f.clone();
                tokio::spawn(async move {
                    let _permit = sem.acquire_owned().await.unwrap();
                    f(item).await
                })
            })
            .collect();
        join_all(handles)
            .await
            .into_iter()
            .filter_map(|r| r.ok())
            .collect()
    }
}
