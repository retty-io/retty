//! Extensible encoder/decoder and its common implementations which deal with the packet fragmentation
//! and reassembly issue found in a stream-based transport such as TCP or datagram-based transport such as UDP.

pub mod byte_to_message_decoder;
pub mod string_codec;
