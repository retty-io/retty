//! The helpful bootstrap APIs which enable an easy implementation of typical client side and server side pipeline initialization.

use crate::channel::Pipeline;
use local_sync::mpsc::unbounded::Tx;
use std::rc::Rc;

pub(crate) mod bootstrap_client_tcp;
pub(crate) mod bootstrap_client_udp;
pub(crate) mod bootstrap_server_tcp;
pub(crate) mod bootstrap_server_udp;

pub use bootstrap_client_tcp::BootstrapClientTcp;
pub use bootstrap_client_udp::BootstrapClientUdp;
pub use bootstrap_server_tcp::BootstrapServerTcp;
pub use bootstrap_server_udp::BootstrapServerUdp;

/// Creates a new [Pipeline]
pub type PipelineFactoryFn<R, W> = Box<dyn (Fn(Tx<R>) -> Rc<Pipeline<R, W>>)>;

const MAX_DURATION_IN_SECS: u64 = 86400; // 1 day
