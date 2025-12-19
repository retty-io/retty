//! # SansIO Codec - Protocol Codecs for SansIO
//!
//! `sansio-codec` provides reusable codec implementations for the sansio ecosystem,
//! including frame decoders, string codecs, and transport abstractions.
//!
//! ## Features
//!
//! - **Frame Decoders**: Line-based frame decoders for parsing delimited streams
//! - **String Codecs**: UTF-8 string encoding/decoding
//! - **Transport Abstractions**: Tagged message types with transport metadata
//! - **Sans-IO Design**: Pure protocol logic without I/O dependencies
//!
//! ## Quick Start
//!
//! ```toml
//! [dependencies]
//! sansio-codec = "0.0.8"
//! ```
//!
//! ## Building a Pipeline with Codecs
//!
//! ```rust,no_run
//! use sansio::Pipeline;
//! use sansio_codec::{
//!     LineBasedFrameDecoder, TaggedByteToMessageCodec, TaggedStringCodec, TerminatorType,
//! };
//! use sansio_transport::{TaggedBytesMut, TaggedString};
//!
//! let mut pipeline = Pipeline::<TaggedBytesMut, TaggedString>::new();
//!
//! // Add line-based frame decoder
//! let frame_decoder = TaggedByteToMessageCodec::new(Box::new(
//!     LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH)
//! ));
//! pipeline.add_back(frame_decoder);
//!
//! // Add string codec
//! pipeline.add_back(TaggedStringCodec::new());
//!
//! // Add your business logic handler
//! // pipeline.add_back(your_handler);
//!
//! pipeline.finalize();
//! ```
//!
//! For shared pipelines (e.g., UDP servers), use `RcPipeline`:
//!
//! ```rust,no_run
//! use sansio::RcPipeline;
//! use sansio_codec::{TaggedByteToMessageCodec, TaggedStringCodec};
//! use sansio_transport::{TaggedBytesMut, TaggedString};
//! use std::rc::Rc;
//!
//! let mut pipeline = RcPipeline::<TaggedBytesMut, TaggedString>::new();
//! // ... add handlers ...
//! pipeline.finalize();
//! let pipeline = Rc::new(pipeline);  // Now shareable
//! ```
//!
//! ## Modules
//!
//! - [`byte_to_message_decoder`]: Frame decoders for parsing byte streams
//! - [`string_codec`]: UTF-8 string encoding/decoding handlers
//!
//! ## Transport Context
//!
//! The transport module provides tagged message types that carry metadata:
//!
//! ```rust
//! use sansio_transport::{TransportContext, TaggedBytesMut, TransportProtocol};
//! use std::time::Instant;
//! use bytes::BytesMut;
//!
//! let msg = TaggedBytesMut {
//!     now: Instant::now(),
//!     transport: TransportContext {
//!         local_addr: "127.0.0.1:8080".parse().unwrap(),
//!         peer_addr: "127.0.0.1:1234".parse().unwrap(),
//!         transport_protocol: TransportProtocol::TCP,
//!         ecn: None,
//!     },
//!     message: BytesMut::from("Hello"),
//! };
//! ```

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/webrtc-rs/sansio/master/doc/sansio-white.png"
)]
#![warn(rust_2018_idioms)]
#![warn(missing_docs)]

/// Byte-to-message frame decoders for parsing delimited streams
pub mod byte_to_message_decoder;

/// UTF-8 string encoding/decoding handlers
pub mod string_codec;

// Re-export commonly used types
pub use byte_to_message_decoder::{
    LineBasedFrameDecoder, MessageDecoder, TaggedByteToMessageCodec, TerminatorType,
};
pub use string_codec::TaggedStringCodec;
