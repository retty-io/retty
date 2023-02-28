use crate::bootstrap::{
    BootstrapClientTcp, BootstrapClientUdp, BootstrapServerTcp, BootstrapServerUdp,
};
use crate::channel::Pipeline;
use crate::codec::string_codec::{StringCodec, TaggedString, TaggedStringCodec};
use crate::runtime::default_runtime;
use crate::transport::{AsyncTransportWrite, TaggedBytesMut};
use anyhow::Result;
use bytes::BytesMut;

#[tokio::test]
async fn bootstrap_basic() -> Result<()> {
    let rt = default_runtime().unwrap();
    let _tcp_server = BootstrapServerTcp::<String>::new(rt.clone());
    let _tcp_client = BootstrapClientTcp::<String>::new(rt.clone());
    let _udp_server = BootstrapServerUdp::<TaggedString>::new(rt.clone());
    let _udp_client = BootstrapClientUdp::<TaggedString>::new(rt.clone());

    Ok(())
}

#[tokio::test]
async fn bootstrap_tcp_server_with_pipeline() -> Result<()> {
    let mut bootstrap = BootstrapServerTcp::<String>::new(default_runtime().unwrap());
    bootstrap.pipeline(Box::new(
        move |_sock: Box<dyn AsyncTransportWrite + Send + Sync>| {
            Box::pin(async move {
                let pipeline: Pipeline<BytesMut, String> = Pipeline::new();
                pipeline.add_back(StringCodec::new()).await;
                pipeline.finalize().await
            })
        },
    ));

    bootstrap.bind(format!("{}:{}", "0.0.0.0", 0)).await?;
    bootstrap.stop().await;

    Ok(())
}

#[tokio::test]
async fn bootstrap_udp_server_with_pipeline() -> Result<()> {
    let mut bootstrap = BootstrapServerUdp::<TaggedString>::new(default_runtime().unwrap());
    bootstrap.pipeline(Box::new(
        move |_sock: Box<dyn AsyncTransportWrite + Send + Sync>| {
            Box::pin(async move {
                let pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();
                pipeline.add_back(TaggedStringCodec::new()).await;
                pipeline.finalize().await
            })
        },
    ));

    bootstrap.bind(format!("{}:{}", "0.0.0.0", 0)).await?;
    bootstrap.stop().await;

    Ok(())
}

#[tokio::test]
async fn bootstrap_tcp_client_with_pipeline() -> Result<()> {
    let mut bootstrap = BootstrapClientTcp::<String>::new(default_runtime().unwrap());
    bootstrap.pipeline(Box::new(
        move |_sock: Box<dyn AsyncTransportWrite + Send + Sync>| {
            Box::pin(async move {
                let pipeline: Pipeline<BytesMut, String> = Pipeline::new();
                pipeline.add_back(StringCodec::new()).await;
                pipeline.finalize().await
            })
        },
    ));

    let result = bootstrap.connect(format!("{}:{}", "0.0.0.0", 8080)).await;
    assert!(result.is_err());

    bootstrap.stop().await;

    Ok(())
}

#[tokio::test]
async fn bootstrap_udp_client_with_pipeline() -> Result<()> {
    let mut bootstrap = BootstrapClientUdp::<TaggedString>::new(default_runtime().unwrap());
    bootstrap.pipeline(Box::new(
        move |_sock: Box<dyn AsyncTransportWrite + Send + Sync>| {
            Box::pin(async move {
                let pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();
                pipeline.add_back(TaggedStringCodec::new()).await;
                pipeline.finalize().await
            })
        },
    ));

    bootstrap.bind(format!("{}:{}", "0.0.0.0", 0)).await?;
    let _ = bootstrap.connect(format!("{}:{}", "0.0.0.0", 8080)).await;
    bootstrap.stop().await;

    Ok(())
}
