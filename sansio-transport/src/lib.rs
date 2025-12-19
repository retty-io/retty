//! Uniform interface to send/recv UDP packets with ECN information.
#![allow(rust_2024_compatibility)]
#![warn(rust_2018_idioms)]
#![allow(dead_code)]
//#![warn(missing_docs)]

#[allow(unused_macros)]
macro_rules! ready {
    ($e:expr $(,)?) => {
        match $e {
            std::task::Poll::Ready(t) => t,
            std::task::Poll::Pending => return std::task::Poll::Pending,
        }
    };
}

#[cfg(unix)]
use std::os::unix::io::AsRawFd;
#[cfg(windows)]
use std::os::windows::io::AsRawSocket;
use std::sync::atomic::AtomicU64;
use std::{
    net::{IpAddr, Ipv6Addr, SocketAddr},
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

use tracing::warn;

#[cfg(unix)]
mod cmsg;
#[cfg(unix)]
#[path = "unix.rs"]
mod imp;

#[cfg(windows)]
#[path = "windows.rs"]
mod imp;

// No ECN support
#[cfg(not(any(unix, windows)))]
#[path = "fallback.rs"]
mod imp;

mod proto;
mod runtime;

pub use imp::UdpSocketState;
pub use proto::{
    EcnCodepoint, FiveTuple, FourTuple, TaggedBytesMut, TaggedString, Transmit, TransportContext,
    TransportMessage, TransportProtocol,
};
pub use runtime::AsyncUdpSocket;
pub use runtime::UdpSocket;

/// Number of UDP packets to send/receive at a time
pub const BATCH_SIZE: usize = imp::BATCH_SIZE;

/// The capabilities a UDP socket supports on a certain platform
#[derive(Debug)]
pub struct Capabilities {
    max_gso_segments: AtomicUsize,
    gro_segments: usize,
}

impl Capabilities {
    pub fn new() -> Self {
        imp::capabilities()
    }

    /// The maximum amount of segments which can be transmitted if a platform
    /// supports Generic Send Offload (GSO).
    ///
    /// This is 1 if the platform doesn't support GSO. Subject to change if errors are detected
    /// while using GSO.
    #[inline]
    pub fn max_gso_segments(&self) -> usize {
        self.max_gso_segments.load(Ordering::Relaxed)
    }

    /// The number of segments to read when GRO is enabled. Used as a factor to
    /// compute the receive buffer size.
    ///
    /// Returns 1 if the platform doesn't support GRO.
    #[inline]
    pub fn gro_segments(&self) -> usize {
        self.gro_segments
    }
}

impl Default for Capabilities {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Copy, Clone)]
pub struct RecvMeta {
    pub addr: SocketAddr,
    pub len: usize,
    pub stride: usize,
    pub ecn: Option<EcnCodepoint>,
    /// The destination IP address which was encoded in this datagram
    pub dst_ip: Option<IpAddr>,
}

impl Default for RecvMeta {
    /// Constructs a value with arbitrary fields, intended to be overwritten
    fn default() -> Self {
        Self {
            addr: SocketAddr::new(Ipv6Addr::UNSPECIFIED.into(), 0),
            len: 0,
            stride: 0,
            ecn: None,
            dst_ip: None,
        }
    }
}

/// Log at most 1 IO error per minute
const IO_ERROR_LOG_INTERVAL: Duration = std::time::Duration::from_secs(60);

/// Logs a warning message when sendmsg fails
///
/// Logging will only be performed if at least [`IO_ERROR_LOG_INTERVAL`]
/// has elapsed since the last error was logged.
fn log_sendmsg_error(
    epoch: &Instant,
    last_send_error: &AtomicU64,
    err: impl core::fmt::Debug,
    transmit: &Transmit,
) {
    let d = last_send_error.load(Ordering::Relaxed);
    let last = epoch.checked_add(Duration::from_nanos(d)).unwrap();
    let now = Instant::now();
    let interval = now.saturating_duration_since(last);
    if interval > IO_ERROR_LOG_INTERVAL {
        last_send_error.store(interval.as_nanos() as u64, Ordering::Relaxed);
        warn!(
            "sendmsg error: {:?}, Transmit: {{ destination: {:?}, src_ip: {:?}, enc: {:?}, len: {:?}, segment_size: {:?} }}",
            err,
            transmit.destination,
            transmit.src_ip,
            transmit.ecn,
            transmit.contents.len(),
            transmit.segment_size
        );
    }
}

/// A borrowed UDP socket
///
/// On Unix, constructible via `From<T: AsRawFd>`. On Windows, constructible via `From<T:
/// AsRawSocket>`.
// Wrapper around socket2 to avoid making it a public dependency and incurring stability risk
pub struct UdpSockRef<'a>(socket2::SockRef<'a>);

#[cfg(unix)]
impl<'s, S> From<&'s S> for UdpSockRef<'s>
where
    S: AsRawFd + std::os::fd::AsFd,
{
    fn from(socket: &'s S) -> Self {
        Self(socket.into())
    }
}

#[cfg(windows)]
impl<'s, S> From<&'s S> for UdpSockRef<'s>
where
    S: AsRawSocket + std::os::windows::io::AsSocket,
{
    fn from(socket: &'s S) -> Self {
        Self(socket.into())
    }
}
