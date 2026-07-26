use std::{
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use tokio::sync::Notify;

#[derive(Debug, Default)]
pub(crate) struct FinalizationTracker {
    active: AtomicUsize,
    idle: Notify,
}

impl FinalizationTracker {
    pub(crate) fn spawn<F>(self: &Arc<Self>, future: F) -> bool
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return false;
        };
        self.active.fetch_add(1, Ordering::AcqRel);
        let tracker = self.clone();
        runtime.spawn(async move {
            let _guard = ActiveFinalization { tracker };
            future.await;
        });
        true
    }

    pub(crate) fn active(&self) -> usize {
        self.active.load(Ordering::Acquire)
    }

    pub(crate) async fn drain(&self, timeout: Duration) -> bool {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if self.active() == 0 {
                return true;
            }
            let notified = self.idle.notified();
            if self.active() == 0 {
                return true;
            }
            if tokio::time::timeout_at(deadline, notified).await.is_err() {
                return self.active() == 0;
            }
        }
    }
}

struct ActiveFinalization {
    tracker: Arc<FinalizationTracker>,
}

impl Drop for ActiveFinalization {
    fn drop(&mut self) {
        if self.tracker.active.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.tracker.idle.notify_waiters();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn drain_waits_for_spawned_finalizers() {
        let tracker = Arc::new(FinalizationTracker::default());
        let release = Arc::new(Notify::new());
        let task_release = release.clone();
        assert!(tracker.spawn(async move {
            task_release.notified().await;
        }));
        assert_eq!(tracker.active(), 1);
        assert!(!tracker.drain(Duration::from_millis(1)).await);

        release.notify_one();
        assert!(tracker.drain(Duration::from_secs(1)).await);
        assert_eq!(tracker.active(), 0);
    }
}
