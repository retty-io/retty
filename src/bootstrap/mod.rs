//! The helpful bootstrap APIs which enable an easy implementation of typical client side and server side pipeline initialization.

use crate::channel::Pipeline;
use glommio::channels::local_channel::LocalSender;
use std::rc::Rc;

//pub(crate) mod bootstrap_client_tcp;
//pub(crate) mod bootstrap_server_tcp;
mod bootstrap_udp;

//pub use bootstrap_client_tcp::BootstrapClientTcp;
//pub use bootstrap_server_tcp::BootstrapServerTcp;
pub use bootstrap_udp::{
    bootstrap_udp_client::BootstrapUdpClient, bootstrap_udp_server::BootstrapUdpServer,
};

/// Creates a new [Pipeline]
pub type PipelineFactoryFn<R, W> = Box<dyn (Fn(LocalSender<R>) -> Rc<Pipeline<R, W>>)>;

const MAX_DURATION_IN_SECS: u64 = 86400; // 1 day
