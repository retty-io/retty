use crate::channel::pipeline::PipelineContext;
use std::future::Future;
use std::pin::Pin;
use tokio::io::AsyncWrite;

pub mod client_bootstrap_tcp;
pub mod server_bootstrap_tcp;

pub type PipelineFactoryFn = Box<
    dyn (Fn(
            Pin<Box<dyn AsyncWrite + Send + Sync>>,
        ) -> Pin<Box<dyn Future<Output = PipelineContext> + Send + 'static>>)
        + Send
        + Sync,
>;
