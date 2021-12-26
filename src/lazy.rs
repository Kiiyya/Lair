//! Simple lazy evaluation, tailored for our use case.
//!
//! The problem:
//! We fetch sources in parallel, and maintain a "Descriptor --> Source path" mapping.
//! But when do we set source path?
//! - If we set it at the beginning of when we start the download, it may not be downloaded before
//!   we start compiling.
//! - If we set it after we finish downloading, we may accidentally start downloading twice.
//!
//! The solution(s):
//! - Download synchronously (lame).
//! - Use lazy evaluation (our choice, because we're fancy like that).
//!
//! So we will maintain a "Descriptor --> Lazy<Source path>" mapping instead, and insert the lazy
//! object immediately, but when users `get()` it, it will block (well, asynchronously block, but
//! whateevr) until it is done downloading.

use std::fmt::Debug;
use std::sync::Arc;

use either::Either;
use futures::Future;
use futures::future::BoxFuture;
use tokio::sync::Mutex;

/// See the module-level docs.
pub struct Lazy<T> {
    inner: Mutex<Either<
        T,
        BoxFuture<'static, T>
    >>,
}

impl<T: Debug> Debug for Lazy<T> {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl<T> Lazy<T> {
    pub fn new<F>(recipe: F) -> Self
    where
        F: Future<Output = T> + Send + 'static,
    {
        Self {
            inner: Mutex::new(Either::Right(Box::pin(recipe))),
        }
    }

    /// Creates a new Lazy which takes an `Arc<S>` when called.
    /// But stores the `arc` as a `Weak<S>` until it is called.
    /// This function is useful to prevent reference counting cycles.
    ///
    /// For now, panics when the Arc-upgrade fails, will maybe change some time in the
    /// future.
    pub fn new_weak<S, Fut, F>(arc: &Arc<S>, recipe: F) -> Self
    where
        S: Send + Sync + 'static,
        Fut: Future<Output = T> + Send + 'static,
        F: (FnOnce(Arc<S>) -> Fut) + Send + 'static,
    {
        let weak = Arc::downgrade(arc);
        let fut = async move {
            let arc = weak.upgrade().expect("Failed to upgrade weak Arc.");
            recipe(arc).await
        };

        Self {
            inner: Mutex::new(Either::Right(Box::pin(fut))),
        }
    }

    pub fn new_immediate(val: T) -> Self {
        Self {
            inner: Mutex::new(Either::Left(val))
        }
    }

    pub async fn get(&self) -> T
        where T: Clone
    {
        let mut guard = self.inner.lock().await;
        match &mut *guard {
            Either::Left(result) => {
                // result is already there, nice!
                result.clone()
            },
            Either::Right(future) => {
                // result is not yet there, but also since we got the lock, it means we're the first.
                // so let's get it!
                let result = future.await;
                *guard = Either::Left(result.clone());
                result
            },
        }
    }

    // pub async fn probe_progress(&self) -> Progress {
    // 	if let Some(x) = self.inner.try
    // }
}

// pub enum Progress {
// 	NotStarted,
// 	Working,
// 	Done,
// }


