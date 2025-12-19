use super::*;

/// A Bootstrap that makes it easy to bootstrap a pipeline to use for UDP clients.
pub struct BootstrapUdpClient<W> {
    bootstrap_udp: BootstrapUdp<W>,
}

impl<W: 'static> Default for BootstrapUdpClient<W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<W: 'static> BootstrapUdpClient<W> {
    /// Creates a new BootstrapUdpClient
    pub fn new() -> Self {
        Self {
            bootstrap_udp: BootstrapUdp::new(),
        }
    }

    /// Sets max payload size, default is 2048 bytes
    pub fn max_payload_size(&mut self, max_payload_size: usize) -> &mut Self {
        self.bootstrap_udp.max_payload_size(max_payload_size);
        self
    }

    /// Creates pipeline instances from when calling [BootstrapUdpClient::bind].
    pub fn pipeline(
        &mut self,
        pipeline_factory_fn: PipelineFactoryFn<TaggedBytesMut, W>,
    ) -> &mut Self {
        self.bootstrap_udp.pipeline(pipeline_factory_fn);
        self
    }

    /// Binds local address and port
    pub async fn bind<A: ToSocketAddrs>(&mut self, addr: A) -> Result<SocketAddr, Error> {
        self.bootstrap_udp.bind(addr).await
    }

    /// Connects to the remote peer
    pub async fn connect(
        &mut self,
        addr: SocketAddr,
    ) -> Result<Rc<dyn RcOutboundPipeline<W>>, Error> {
        self.bootstrap_udp.connect(Some(addr)).await
    }

    /// Stops the client
    pub async fn stop(&self) {
        self.bootstrap_udp.stop().await
    }

    /// Waits for stop of the client
    pub async fn wait_for_stop(&self) {
        self.bootstrap_udp.wait_for_stop().await
    }

    /// Gracefully stop the client
    pub async fn graceful_stop(&self) {
        self.bootstrap_udp.graceful_stop().await
    }
}
