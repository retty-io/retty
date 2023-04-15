use super::*;

/// A Bootstrap that makes it easy to bootstrap a pipeline to use for TCP servers.
pub struct BootstrapTcpServer<W> {
    bootstrap_tcp: BootstrapTcp<W>,
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
            bootstrap_tcp: BootstrapTcp::new(),
        }
    }

    /// Set ThreadPool for accept_group
    pub fn accept_group(&mut self, accept_group: ThreadPool) -> &mut Self {
        self.bootstrap_tcp.accept_group(accept_group);
        self
    }

    /// Set ThreadPool for io_group
    pub fn io_group(&mut self, io_group: ThreadPool) -> &mut Self {
        self.bootstrap_tcp.io_group(io_group);
        self
    }

    /// Creates pipeline instances from when calling [BootstrapTcpServer::bind].
    pub fn pipeline(
        &mut self,
        pipeline_factory_fn: PipelineFactoryFn<TaggedBytesMut, W>,
    ) -> &mut Self {
        self.bootstrap_tcp.pipeline(pipeline_factory_fn);
        self
    }

    /// Binds local address and port
    pub async fn bind<A: AsyncToSocketAddrs>(&self, addr: A) -> Result<SocketAddr, Error> {
        self.bootstrap_tcp.bind(addr).await
    }

    /// Gracefully stop the server
    pub async fn stop(&self) {
        self.bootstrap_tcp.stop().await
    }
}
