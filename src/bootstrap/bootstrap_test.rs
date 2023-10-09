use crate::bootstrap::*;
use crate::channel::*;
use crate::codec::string_codec::*;
use crate::executor::*;
use crate::transport::*;
use std::str::FromStr;

use anyhow::Result;

#[test]
fn bootstrap_basic() -> Result<()> {
    let _tcp_server = BootstrapTcpServer::<String>::new();
    let _tcp_client = BootstrapTcpClient::<String>::new();
    let _udp_server = BootstrapUdpServer::<TaggedString>::new();
    let _udp_client = BootstrapUdpClient::<TaggedString>::new();

    Ok(())
}

#[test]
fn bootstrap_tcp_server_with_pipeline() -> Result<()> {
    LocalExecutorBuilder::default().run(async {
        let mut bootstrap = BootstrapTcpServer::<TaggedString>::new();
        bootstrap.pipeline(Box::new(
            move |_writer: AsyncTransportWrite<TaggedBytesMut>| {
                let pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();
                pipeline.add_back(TaggedStringCodec::new())?;
                pipeline.finalize()
            },
        ));

        bootstrap
            .bind(format!("{}:{}", "0.0.0.0", 0))
            .await
            .unwrap();
        bootstrap.graceful_stop().await;
    });

    Ok(())
}

#[test]
fn bootstrap_udp_server_with_pipeline() -> Result<()> {
    LocalExecutorBuilder::default().run(async {
        let mut bootstrap = BootstrapUdpServer::<TaggedString>::new();
        bootstrap.pipeline(Box::new(
            move |_writer: AsyncTransportWrite<TaggedBytesMut>| {
                let pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();
                pipeline.add_back(TaggedStringCodec::new())?;
                pipeline.finalize()
            },
        ));

        bootstrap
            .bind(format!("{}:{}", "0.0.0.0", 0))
            .await
            .unwrap();
        bootstrap.graceful_stop().await;
    });

    Ok(())
}

#[test]
fn bootstrap_tcp_client_with_pipeline() -> Result<()> {
    LocalExecutorBuilder::default().run(async {
        let mut bootstrap = BootstrapTcpClient::<TaggedString>::new();
        bootstrap.pipeline(Box::new(
            move |_writer: AsyncTransportWrite<TaggedBytesMut>| {
                let pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();
                pipeline.add_back(TaggedStringCodec::new())?;
                pipeline.finalize()
            },
        ));

        let result = bootstrap.connect(format!("{}:{}", "0.0.0.0", 8080)).await;
        assert!(result.is_err());

        bootstrap.graceful_stop().await;
    });

    Ok(())
}

#[test]
fn bootstrap_udp_client_with_pipeline() -> Result<()> {
    LocalExecutorBuilder::default().run(async {
        let mut bootstrap = BootstrapUdpClient::<TaggedString>::new();
        bootstrap.pipeline(Box::new(
            move |_writer: AsyncTransportWrite<TaggedBytesMut>| {
                let pipeline: Pipeline<TaggedBytesMut, TaggedString> = Pipeline::new();
                pipeline.add_back(TaggedStringCodec::new())?;
                pipeline.finalize()
            },
        ));

        bootstrap
            .bind(format!("{}:{}", "0.0.0.0", 0))
            .await
            .unwrap();
        let _ = bootstrap
            .connect(SocketAddr::from_str(&format!("{}:{}", "0.0.0.0", 8080)).unwrap())
            .await
            .unwrap();

        bootstrap.graceful_stop().await;
    });

    Ok(())
}
