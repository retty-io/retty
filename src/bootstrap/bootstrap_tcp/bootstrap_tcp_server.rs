use super::*;

/// A Bootstrap that makes it easy to bootstrap a pipeline to use for TCP servers.
pub struct BootstrapTcpServer<W> {
    internal: BootstrapTcp<W>,
}

impl<W: 'static> Default for BootstrapTcpServer<W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<W: 'static> BootstrapTcpServer<W> {
    /// Creates a new BootstrapTcpServer
    pub fn new() -> Self {
        Self {
            internal: BootstrapTcp::new(),
        }
    }

    /// Creates pipeline instances from when calling [BootstrapTcpServer::bind].
    pub fn pipeline(
        &mut self,
        pipeline_factory_fn: PipelineFactoryFn<TaggedBytesMut, W>,
    ) -> &mut Self {
        self.internal.pipeline(pipeline_factory_fn);
        self
    }

    /// Binds local address and port
    pub async fn bind<A: AsyncToSocketAddrs>(&self, addr: A) -> Result<SocketAddr, Error> {
        self.internal.bind(addr).await
    }

    /// Gracefully stop the server
    pub async fn stop(&self) {
        self.internal.stop().await
    }
}
