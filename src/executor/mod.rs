//! Async executors.

use scoped_tls::scoped_thread_local;
use smol::{LocalExecutor, Task};
use std::future::Future;

scoped_thread_local!(static LOCAL_EX: LocalExecutor<'_>);

/// A factory that can be used to configure and create a [`LocalExecutor`].
#[derive(Debug, Default)]
pub struct LocalExecutorBuilder {}

impl LocalExecutorBuilder {
    /// Creates a new LocalExecutorBuilder
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs the local executor until the given future completes.
    pub fn run<T>(self, f: impl Future<Output = T>) -> T {
        let local_ex = LocalExecutor::new();
        LOCAL_EX.set(&local_ex, || {
            futures_lite::future::block_on(local_ex.run(f))
        })
    }
}

/// Spawns a task onto the current single-threaded executor.
///
/// If called from a [`LocalExecutor`], the task is spawned on it.
/// Otherwise, this method panics.
pub fn spawn_local<T: 'static>(future: impl Future<Output = T> + 'static) -> Task<T> {
    if LOCAL_EX.is_set() {
        LOCAL_EX.with(|local_ex| local_ex.spawn(future))
    } else {
        panic!("`spawn_local()` must be called from a `LocalExecutor`")
    }
}

/// Attempts to yield local to run a task if at least one is scheduled.
/// Running a scheduled task means simply polling its future once.
pub fn try_yield_local() -> bool {
    if LOCAL_EX.is_set() {
        LOCAL_EX.with(|local_ex| local_ex.try_tick())
    } else {
        panic!("`try_yield_local()` must be called from a `LocalExecutor`")
    }
}

/// Yield local to run other tasks until there is no other pending task.
pub fn yield_local() {
    if LOCAL_EX.is_set() {
        LOCAL_EX.with(|local_ex| while local_ex.try_tick() {})
    } else {
        panic!("`try_yield_local()` must be called from a `LocalExecutor`")
    }
}
