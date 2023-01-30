use super::*;
use crate::runtime::default_runtime;

use crate::codec::string_codec::{StringCodec, TaggedString, TaggedStringCodec};
use crate::transport::TaggedBytesMut;
use anyhow::Result;
use bytes::BytesMut;

#[tokio::test]
async fn bootstrap_basic() -> Result<()> {
    let rt = default_runtime().unwrap();
    let _tcp_server = BootstrapTcpServer::<String>::new(rt.clone());
    let _tcp_client = BootstrapTcpClient::<String>::new(rt.clone());
    let _udp_server = BootstrapUdpServer::<TaggedString>::new(rt.clone());
    let _udp_client = BootstrapUdpClient::<TaggedString>::new(rt.clone());

    Ok(())
}

#[tokio::test]
async fn bootstrap_tcp_server_with_pipeline() -> Result<()> {
    let mut bootstrap = BootstrapTcpServer::<String>::new(default_runtime().unwrap());
    bootstrap.pipeline(Box::new(
        move |_sock: Box<dyn AsyncTransportWrite + Send + Sync>| {
            Box::pin(async move {
                let pipeline: Pipeline<BytesMut, String> = Pipeline::new();
                pipeline.add_back(StringCodec::new()).await;
                pipeline.finalize().await;
                Arc::new(pipeline)
            })
        },
    ));

    bootstrap.bind(format!("{}:{}", "0.0.0.0", 0)).await?;
    bootstrap.stop().await;

    Ok(())
}

#[tokio::test]
async fn bootstrap_udp_server_with_pipeline() -> Result<()> {
    let mut bootstrap = BootstrapUdpServer::<TaggedString>::new(default_runtime().unwrap());
    bootstrap.pipeline(Box::new(
        move |_sock: Box<dyn AsyncTransportWrite + Send + Sync>| {
            Box::pin(async move {
                let pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();
                pipeline.add_back(TaggedStringCodec::new()).await;
                pipeline.finalize().await;
                Arc::new(pipeline)
            })
        },
    ));

    bootstrap.bind(format!("{}:{}", "0.0.0.0", 0)).await?;
    bootstrap.stop().await;

    Ok(())
}

#[tokio::test]
async fn bootstrap_tcp_client_with_pipeline() -> Result<()> {
    let mut bootstrap = BootstrapTcpClient::<String>::new(default_runtime().unwrap());
    bootstrap.pipeline(Box::new(
        move |_sock: Box<dyn AsyncTransportWrite + Send + Sync>| {
            Box::pin(async move {
                let pipeline: Pipeline<BytesMut, String> = Pipeline::new();
                pipeline.add_back(StringCodec::new()).await;
                pipeline.finalize().await;
                Arc::new(pipeline)
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
    let mut bootstrap = BootstrapUdpClient::<TaggedString>::new(default_runtime().unwrap());
    bootstrap.pipeline(Box::new(
        move |_sock: Box<dyn AsyncTransportWrite + Send + Sync>| {
            Box::pin(async move {
                let pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();
                pipeline.add_back(TaggedStringCodec::new()).await;
                pipeline.finalize().await;
                Arc::new(pipeline)
            })
        },
    ));

    bootstrap.bind(format!("{}:{}", "0.0.0.0", 0)).await?;
    let _ = bootstrap.connect(format!("{}:{}", "0.0.0.0", 8080)).await;
    bootstrap.stop().await;

    Ok(())
}
