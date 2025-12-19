//! # SansIO Executor - Tokio-based Executor
//!
//! `sansio-executor` provides a tokio-based local executor for the sansio ecosystem.
//!
//! ## Features
//!
//! - **Tokio LocalSet**: Built on tokio's LocalSet for single-threaded task execution
//! - **CPU Pinning**: Pin executor threads to specific CPU cores
//! - **Thread Naming**: Name executor threads for debugging
//! - **Task Management**: Spawn, detach, and cancel tasks
//!
//! ## Quick Start
//!
//! ```toml
//! [dependencies]
//! sansio-executor = "0.0.7"
//! ```
//!
//! ```rust,no_run
//! use sansio_executor::LocalExecutorBuilder;
//!
//! LocalExecutorBuilder::default()
//!     .run(async {
//!         println!("Running on tokio!");
//!     });
//! ```
//!
//! ## CPU Pinning
//!
//! ```rust,no_run
//! use sansio_executor::LocalExecutorBuilder;
//! use core_affinity::CoreId;
//!
//! LocalExecutorBuilder::new()
//!     .name("my-executor")
//!     .core_id(CoreId { id: 0 })
//!     .run(async {
//!         println!("Running on CPU core 0!");
//!     });
//! ```
//!
//! ## Spawning Local Tasks
//!
//! ```rust,no_run
//! use sansio_executor::{LocalExecutorBuilder, spawn_local};
//!
//! LocalExecutorBuilder::default().run(async {
//!     let task1 = spawn_local(async {
//!          println!("Task 1");
//!          42
//!     });
//!
//!     let task2 = spawn_local(async {
//!          println!("Task 2");
//!          100
//!     });
//!
//!     let result1 = task1.await.unwrap();
//!     let result2 = task2.await.unwrap();
//!
//!     println!("Results: {}, {}", result1, result2);
//! });
//! ```
//!
//! ## API
//!
//! The main exports are:
//!
//! - [`LocalExecutorBuilder`]: Builder for configuring and creating executors
//! - [`spawn_local()`]: Spawn a task on the current executor, returns a [`Task`]
//! - [`yield_local()`]: Yield to other tasks
//! - [`Task<T>`]: A handle to a spawned task, implements `Future<Output = Result<T, TaskError>>`
//! - [`TaskError`]: Error type returned when a task fails (panic or cancellation)
//!
//! ## Task Execution
//!
//! Tasks spawned with [`spawn_local()`] return a [`Task<T>`] handle that can be awaited:
//!
//! ```rust,no_run
//! use sansio_executor::{LocalExecutorBuilder, spawn_local};
//!
//! LocalExecutorBuilder::default().run(async {
//!     let task = spawn_local(async { 42 });
//!
//!     // Task implements Future<Output = Result<T, TaskError>>
//!     match task.await {
//!         Ok(result) => println!("Task completed: {}", result),
//!         Err(e) => eprintln!("Task failed: {}", e),
//!     }
//! });
//! ```
//!
//! ## Detaching Tasks
//!
//! Tasks can be detached to run in the background without awaiting them:
//!
//! ```rust,no_run
//! use sansio_executor::{LocalExecutorBuilder, spawn_local};
//!
//! LocalExecutorBuilder::default().run(async {
//!     let task = spawn_local(async {
//!         println!("Running in background");
//!     });
//!
//!     // Detach - task continues running even though we don't await it
//!     task.detach();
//! });
//! ```
//!
//! ## Error Handling
//!
//! Tasks can fail if they panic or are cancelled/aborted. The [`TaskError`] type
//! wraps tokio's `JoinError` and provides error information.

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/webrtc-rs/sansio/master/doc/sansio-white.png"
)]
#![warn(rust_2018_idioms)]
#![allow(dead_code)]
#![warn(missing_docs)]

mod tokio;
pub use tokio::*;
