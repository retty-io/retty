# sansio

An IO-free Rust networking framework that makes it easy to build protocols, application clients, and servers using the Sans-IO pattern.

Inspired by [Netty](https://netty.io) and [Wangle](https://github.com/facebook/wangle), `sansio` brings the power of pipeline-based protocol composition to Rust.

## Features

- **Sans-IO Design**: Complete separation of protocol logic from I/O operations
- **Pipeline Architecture**: Chain handlers to build complex protocols from simple components
- **Type-Safe Composition**: Rust's type system ensures handlers connect correctly
- **Runtime Agnostic**: Works with tokio async runtime
- **Testable**: Test protocol logic without any I/O

## Core Concepts

### Pipeline Variants

Sansio provides three pipeline variants for different use cases:

- **`Pipeline`**: Exclusive ownership (`&mut self`). Zero overhead, ideal for TCP where each connection owns its pipeline.
- **`RcPipeline`**: Shared ownership with `Rc`. Methods take `&self`, perfect for UDP servers where one pipeline handles messages from multiple peers.
- **`ArcPipeline`**: Thread-safe shared ownership with `Arc` and `Mutex`. Use for multi-threaded servers.

```rust
use sansio::Pipeline;

let mut pipeline = Pipeline::new();
pipeline.add_back(frame_decoder);
pipeline.add_back(string_codec);
pipeline.add_back(business_logic);
pipeline.finalize();
```

For shared pipelines:

```rust
use sansio::RcPipeline;
use std::rc::Rc;

let mut pipeline = RcPipeline::new();
// ... add handlers ...
pipeline.finalize();
let pipeline = Rc::new(pipeline);  // Now shareable
```

### Handler

Each `Handler` processes messages flowing through the pipeline with four associated types:
- `Rin`: Input type for inbound messages
- `Rout`: Output type for inbound messages
- `Win`: Input type for outbound messages
- `Wout`: Output type for outbound messages

```rust
use sansio::{Handler, Context};

impl Handler for MyHandler {
    type Rin = String;
    type Rout = String;
    type Win = String;
    type Wout = String;

    fn handle_read(&mut self, ctx: &Context<...>, msg: Self::Rin) {
        // Process inbound message
        ctx.fire_handle_read(msg);
    }
}
```

### Protocol

The `Protocol` trait provides a simpler, fully Sans-IO alternative for protocol implementation:

```rust
use sansio::Protocol;

impl Protocol for MyProtocol {
    type Rin = Vec<u8>;
    type Rout = Message;
    type Win = Message;
    type Wout = Vec<u8>;

    fn handle_read(&mut self, msg: Self::Rin) -> Option<Self::Rout> {
        // Parse bytes into messages
        Some(parse(msg))
    }
}
```

## Quick Start

Add `sansio` to your `Cargo.toml`:

```toml
[dependencies]
sansio = "0.0.5"
```

Build a simple pipeline:

```rust
use sansio::{Pipeline, Handler, Context};

// Define your handler
struct EchoHandler;

impl Handler for EchoHandler {
    type Rin = String;
    type Rout = String;
    type Win = String;
    type Wout = String;

    fn name(&self) -> &str { "EchoHandler" }

    fn handle_read(&mut self, ctx: &Context<...>, msg: Self::Rin) {
        println!("Received: {}", msg);
        // Echo back
        ctx.write(msg);
    }

    fn poll_write(&mut self, ctx: &Context<...>) -> Option<Self::Wout> {
        ctx.fire_poll_write()
    }
}

fn main() {
    let mut pipeline = Pipeline::new();
    pipeline.add_back(EchoHandler);
    pipeline.finalize();

    pipeline.transport_active();
    pipeline.handle_read("Hello".to_string());

    // Process outbound messages
    while let Some(msg) = pipeline.poll_write() {
        println!("Sending: {}", msg);
    }
}
```

## Using with a Runtime

`sansio` is purely Sans-IO without any runtime dependencies. For async runtime support, use the separate `sansio-executor` crate:

```toml
[dependencies]
sansio = "0.0.7"
sansio-executor = "0.0.7"  # For runtime support
```

```rust
use sansio_executor::LocalExecutorBuilder;

fn main() {
    LocalExecutorBuilder::default()
        .run(async {
            // Your async code here
        });
}
```

See the [sansio-executor documentation](https://docs.rs/sansio-executor) for more details on runtime support.

## Documentation

- [API Documentation](https://docs.rs/sansio)
- [Examples](../examples)
- [Executor (sansio-executor)](https://docs.rs/sansio-executor)

## Examples

See the [`examples`](../examples) crate for complete working examples:

- UDP chat server with pipeline-based protocol processing
- Handler composition and shared state management

Run an example:

```bash
cargo run --example chat_server_udp -p examples
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](../LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
