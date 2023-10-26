use clap::Parser;
use kcp_rust::Config;
use std::{net::SocketAddr, str::FromStr, sync::Arc};

use kcp_tunnel::KcpRuntimeWithTokio;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    // tunnel listen address
    tunnel: SocketAddr,
    // forward to address
    to: SocketAddr,
    // work thread number
    #[arg(short, default_value_t = 6)]
    works: usize,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 6)]
async fn main() -> std::io::Result<()> {
    env_logger::builder()
        .default_format()
        // .filter_module("kcp_rust", log::LevelFilter::Trace)
        .filter_level(log::LevelFilter::Trace)
        .init();

    // let args = Args::parse();

    let config = Config::default();
    let args = Args {
        to: SocketAddr::from_str("192.168.233.128:8888").unwrap(),
        tunnel: SocketAddr::from_str("0.0.0.0:8080").unwrap(),
        works: 0,
    };

    let works = args.works.min(6);

    let kcp_listener = kcp_rust::KcpListener::new::<KcpRuntimeWithTokio>(
        {
            let udp = tokio::net::UdpSocket::bind(args.tunnel).await?;
            kcp_tunnel::TunnelSocket(Arc::new(udp))
        },
        config,
    )?;

    log::info!("work threads: {}", works);
    log::info!("kcp listener started: {}", args.tunnel);
    log::info!("all connections will be forwarded to {}", args.to);

    loop {
        let (kcp_conv, addr, kcp_stream) = kcp_listener.accept().await?;
        let kcp_stream = kcp_tunnel::KcpTunnelStream(kcp_stream);

        let tcp_stream = match tokio::net::TcpStream::connect(args.to).await {
            Ok(stream) => stream,
            Err(e) => {
                log::warn!("forwarding failed: {}", e);
                continue;
            }
        };

        tokio::spawn(async move {
            let addr = format!("{} -> {}({})", args.to, addr, kcp_conv);

            log::debug!("start forward {}", addr);

            let (mut tcp_reader, mut tcp_writer) = tokio::io::split(tcp_stream);
            let (mut kcp_reader, mut kcp_writer) = tokio::io::split(kcp_stream);

            tokio::select!(
                _ = kcp_tunnel::copy(&mut kcp_reader, &mut tcp_writer) => {

                },
                _ = kcp_tunnel::copy(&mut tcp_reader, &mut kcp_writer) => {

                },
            );

            log::debug!("close stream {}", addr);
        });
    }
}
