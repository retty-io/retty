use super::*;

/// A Bootstrap that makes it easy to bootstrap a pipeline to use for UDP servers.
pub struct BootstrapUdpServer<W> {
    internal: BootstrapUdp<W>,
}

impl<W: 'static> Default for BootstrapUdpServer<W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<W: 'static> BootstrapUdpServer<W> {
    /// Creates a new BootstrapUdpServer
    pub fn new() -> Self {
        Self {
            internal: BootstrapUdp::new(),
        }
    }

    /// Creates pipeline instances from when calling [BootstrapUdpServer::bind].
    pub fn pipeline(
        &mut self,
        pipeline_factory_fn: PipelineFactoryFn<TaggedBytesMut, W>,
    ) -> &mut Self {
        self.internal.pipeline(pipeline_factory_fn);
        self
    }

    /// Binds local address and port, return local socket address
    pub fn bind<A: ToSocketAddrs>(&mut self, addr: A) -> Result<SocketAddr, Error> {
        let local_addr = self.internal.bind(addr)?;
        let _ = self.internal.serve()?;
        Ok(local_addr)
    }

    /// Gracefully stop the server
    pub async fn stop(&self) {
        self.internal.stop().await;
    }
}
