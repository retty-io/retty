//! Tokio-based local executor implementation
//!
//! This module provides a local executor implementation using tokio's `LocalSet`.

use core_affinity::{CoreId, set_for_current};
use scoped_tls::scoped_thread_local;
use std::{
    future::Future,
    io::Result,
    pin::Pin,
    task::{Context, Poll},
    thread::{self, JoinHandle},
};
use tokio::task::LocalSet;

scoped_thread_local!(pub(super) static LOCAL: LocalSet);

/// A handle to a spawned task.
///
/// This is a wrapper around tokio's `JoinHandle` that provides a consistent API.
///
/// When awaited, returns `Result<T, TaskError>`:
/// - `Ok(T)`: The task completed successfully
/// - `Err(TaskError)`: The task panicked or was aborted/cancelled
///
/// # Example
///
/// ```rust,no_run
/// use sansio_executor::{LocalExecutorBuilder, spawn_local};
///
/// LocalExecutorBuilder::default().run(async {
///     let task = spawn_local(async { 42 });
///     let result = task.await.unwrap();
///     assert_eq!(result, 42);
/// });
/// ```
pub struct Task<T> {
    inner: tokio::task::JoinHandle<T>,
}

impl<T> Future for Task<T> {
    type Output = std::result::Result<T, TaskError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.inner)
            .poll(cx)
            .map(|result| result.map_err(|e| TaskError { inner: e }))
    }
}

impl<T> Task<T> {
    /// Detaches the task, allowing it to run in the background.
    ///
    /// This consumes the task handle and allows the task to continue running
    /// without being awaited. The task will run to completion in the background.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use sansio_executor::{LocalExecutorBuilder, spawn_local};
    ///
    /// LocalExecutorBuilder::default().run(async {
    ///     let task = spawn_local(async {
    ///         println!("Running in background");
    ///     });
    ///
    ///     // Detach the task - it continues running
    ///     task.detach();
    ///
    ///     // We can't await it anymore, but it will complete
    /// });
    /// ```
    pub fn detach(self) {
        // In tokio, dropping the JoinHandle allows the task to continue running
        drop(self.inner);
    }

    /// Cancels the task.
    ///
    /// This immediately aborts the task's execution.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use sansio_executor::{LocalExecutorBuilder, spawn_local};
    ///
    /// LocalExecutorBuilder::default().run(async {
    ///     let task = spawn_local(async {
    ///         println!("This will not print");
    ///     });
    ///
    ///     // Cancel the task
    ///     task.cancel();
    /// });
    /// ```
    pub fn cancel(self) {
        self.inner.abort();
    }
}

/// Error returned when a spawned task fails.
///
/// This can occur when:
/// - The task panics
/// - The task is aborted/cancelled
#[derive(Debug)]
pub struct TaskError {
    inner: tokio::task::JoinError,
}

impl std::fmt::Display for TaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl std::error::Error for TaskError {}

/// A factory that can be used to configure and create a tokio [`LocalSet`].
#[derive(Debug, Default)]
pub struct LocalExecutorBuilder {
    core_id: Option<CoreId>,
    name: String,
}

impl LocalExecutorBuilder {
    /// Creates a new LocalExecutorBuilder
    pub fn new() -> Self {
        Self::default()
    }

    /// Names the thread-to-be. Currently, the name is used for identification only in panic messages.
    pub fn name(mut self, name: &str) -> Self {
        self.name = String::from(name);
        self
    }

    /// Pins the thread to the specified CPU core
    pub fn core_id(mut self, core_id: CoreId) -> Self {
        self.core_id = Some(core_id);
        self
    }

    /// Runs the local executor on the current thread until the given future completes.
    pub fn run<T>(mut self, f: impl Future<Output = T>) -> T {
        if let Some(core_id) = self.core_id.take() {
            set_for_current(core_id);
        }

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to build tokio runtime");

        let local_set = LocalSet::new();
        LOCAL.set(&local_set, || rt.block_on(local_set.run_until(f)))
    }

    /// Spawns a thread to run the local executor until the given future completes.
    pub fn spawn<G, F, T>(mut self, fut_gen: G) -> Result<JoinHandle<T>>
    where
        G: FnOnce() -> F + Send + 'static,
        F: Future<Output = T> + 'static,
        T: Send + 'static,
    {
        let mut core_id = self.core_id.take();

        thread::Builder::new().name(self.name).spawn(move || {
            if let Some(core_id) = core_id.take() {
                set_for_current(core_id);
            }

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build tokio runtime");

            let local_set = LocalSet::new();
            LOCAL.set(&local_set, || rt.block_on(local_set.run_until(fut_gen())))
        })
    }
}

/// Spawns a task onto the current single-threaded executor.
///
/// If called from a tokio [`LocalSet`], the task is spawned on it.
/// Otherwise, this method panics.
///
/// Returns a [`Task<T>`] that implements `Future<Output = Result<T, TaskError>>`.
/// The task can be awaited to retrieve its result.
///
/// # Panics
///
/// Panics if called outside of a `LocalSet` context.
///
/// # Example
///
/// ```rust,no_run
/// use sansio_executor::{LocalExecutorBuilder, spawn_local};
///
/// LocalExecutorBuilder::default().run(async {
///     let task1 = spawn_local(async { 1 + 1 });
///     let task2 = spawn_local(async { 2 + 2 });
///
///     let result1 = task1.await.unwrap();
///     let result2 = task2.await.unwrap();
///
///     println!("Results: {}, {}", result1, result2);
/// });
/// ```
pub fn spawn_local<T: 'static>(future: impl Future<Output = T> + 'static) -> Task<T> {
    if LOCAL.is_set() {
        LOCAL.with(|local_set| Task {
            inner: local_set.spawn_local(future),
        })
    } else {
        panic!("`spawn_local()` must be called from a tokio `LocalSet`")
    }
}

/// Yields to allow other tasks in the same executor to run.
///
/// This is an async function. Call it as `yield_local().await`.
///
/// This function yields execution to allow other tasks in the same LocalSet to run.
///
/// # Example
///
/// ```rust,no_run
/// use sansio_executor::{LocalExecutorBuilder, spawn_local, yield_local};
///
/// LocalExecutorBuilder::default().run(async {
///     spawn_local(async {
///         println!("Task 1 starting");
///         yield_local().await;  // Let other tasks run
///         println!("Task 1 resuming");
///     });
///
///     spawn_local(async {
///         println!("Task 2 running");
///     });
/// });
/// ```
pub async fn yield_local() {
    tokio::task::yield_now().await
}
