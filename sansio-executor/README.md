# sansio-executor

Tokio-based local executor for the `sansio` ecosystem.

## Features

- **Tokio LocalSet**: Built on tokio's LocalSet for single-threaded task execution
- **CPU Pinning**: Pin executor threads to specific CPU cores
- **Thread Naming**: Name executor threads for debugging
- **Task Management**: Spawn, detach, and cancel tasks

## Quick Start

```toml
[dependencies]
sansio-executor = "0.0.7"
```

```rust
use sansio_executor::LocalExecutorBuilder;

fn main() {
    LocalExecutorBuilder::default()
        .run(async {
            println!("Running on tokio!");
        });
}
```

## Spawning Tasks

```rust
use sansio_executor::{LocalExecutorBuilder, spawn_local};

fn main() {
    LocalExecutorBuilder::default().run(async {
        let task1 = spawn_local(async {
            println!("Task 1");
            42
        });

        let task2 = spawn_local(async {
            println!("Task 2");
            100
        });

        let result1 = task1.await.unwrap();
        let result2 = task2.await.unwrap();

        println!("Results: {}, {}", result1, result2);
    });
}
```

## Detaching Tasks

```rust
use sansio_executor::{LocalExecutorBuilder, spawn_local};

fn main() {
    LocalExecutorBuilder::default().run(async {
        let task = spawn_local(async {
            println!("Running in background");
        });

        // Detach - task continues running even though we don't await it
        task.detach();
    });
}
```

## Documentation

For detailed documentation, see:
- [API Documentation](https://docs.rs/sansio-executor)
- [Local Executor Guide](doc/LocalExecutor.md)
- [Main sansio crate](https://docs.rs/sansio)

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](../LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
