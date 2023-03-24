//! The helpful bootstrap APIs which enable an easy implementation of typical client side and server side pipeline initialization.

use crate::channel::Pipeline;
use local_sync::mpsc::unbounded::Tx as LocalSender;
use std::rc::Rc;

//mod bootstrap_tcp;
mod bootstrap_udp;

/*pub use bootstrap_tcp::{
    bootstrap_tcp_client::BootstrapTcpClient, bootstrap_tcp_server::BootstrapTcpServer,
};*/
pub use bootstrap_udp::{
    bootstrap_udp_client::BootstrapUdpClient, bootstrap_udp_server::BootstrapUdpServer,
};

/// Creates a new [Pipeline]
pub type PipelineFactoryFn<R, W> = Box<dyn (Fn(LocalSender<R>) -> Rc<Pipeline<R, W>>)>;

const MAX_DURATION_IN_SECS: u64 = 86400; // 1 day
