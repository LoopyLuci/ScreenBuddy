//! Thread Pool for ScreenBuddy
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskPriority {
    Realtime,
    High,
    Normal,
    Low,
}

#[derive(Debug, Default)]
pub struct PoolMetrics {
    pub tasks_completed: AtomicU64,
    pub tasks_queued: AtomicU64,
}

impl PoolMetrics {
    pub fn completed(&self) -> u64 {
        self.tasks_completed.load(Ordering::Relaxed)
    }
    pub fn queued(&self) -> u64 {
        self.tasks_queued.load(Ordering::Relaxed)
    }
}

pub struct ThreadPool {
    sender: crossbeam_channel::Sender<Box<dyn FnOnce() + Send>>,
    handles: Vec<thread::JoinHandle<()>>,
    metrics: Arc<PoolMetrics>,
    running: Arc<AtomicBool>,
}

impl Default for ThreadPool {
    fn default() -> Self {
        Self::new()
    }
}

impl ThreadPool {
    pub fn new() -> Self {
        let n = std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(4);
        let (sender, receiver) = crossbeam_channel::unbounded::<Box<dyn FnOnce() + Send>>();
        let metrics = Arc::new(PoolMetrics::default());
        let running = Arc::new(AtomicBool::new(true));
        let mut handles = Vec::with_capacity(n);

        for id in 0..n {
            let rx = receiver.clone();
            let m = metrics.clone();
            let r = running.clone();
            handles.push(
                thread::Builder::new()
                    .name(format!("sb-worker-{}", id))
                    .spawn(move || {
                        while r.load(Ordering::Relaxed) {
                            match rx.recv_timeout(Duration::from_millis(100)) {
                                Ok(f) => {
                                    f();
                                    m.tasks_completed.fetch_add(1, Ordering::Relaxed);
                                }
                                Err(_) => continue,
                            }
                        }
                    })
                    .unwrap(),
            );
        }
        Self {
            sender,
            handles,
            metrics,
            running,
        }
    }

    pub fn spawn<F>(&self, _priority: TaskPriority, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.metrics.tasks_queued.fetch_add(1, Ordering::Relaxed);
        let _ = self.sender.send(Box::new(f));
    }

    pub fn metrics(&self) -> &PoolMetrics {
        &self.metrics
    }

    pub fn shutdown(self) {
        self.running.store(false, Ordering::SeqCst);
        for h in self.handles {
            let _ = h.join();
        }
    }
}

pub fn global_pool() -> &'static ThreadPool {
    use std::sync::OnceLock;
    static POOL: OnceLock<ThreadPool> = OnceLock::new();
    POOL.get_or_init(ThreadPool::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool() {
        let pool = ThreadPool::new();
        let (tx, rx) = crossbeam_channel::bounded::<()>(1);
        pool.spawn(TaskPriority::High, move || {
            tx.send(()).ok();
        });
        std::thread::sleep(Duration::from_millis(50));
        assert!(rx.try_recv().is_ok());
    }
}
