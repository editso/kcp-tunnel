use std::{future::Future, pin::Pin, sync::Arc};

use kcp_rust::AsyncWrite;
use tokio::io::ReadBuf;

#[derive(Clone)]
pub struct TunnelSocket(pub Arc<tokio::net::UdpSocket>);

pub struct KcpRuntimeWithTokio;

pub struct KcpRunnerWithTokio;

pub struct KcpTimerWithTokio;

pub struct KcpTunnelStream<K>(pub kcp_rust::KcpStream<K>);

type BoxedFuture<O> = Pin<Box<dyn Future<Output = O> + Send + 'static>>;

impl kcp_rust::AsyncRecvfrom for TunnelSocket {
    fn poll_recvfrom(
        &mut self,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<std::io::Result<(std::net::SocketAddr, usize)>> {
        let mut buf = ReadBuf::new(buf);
        match self.0.poll_recv_from(cx, &mut buf)? {
            std::task::Poll::Pending => std::task::Poll::Pending,
            std::task::Poll::Ready(addr) => std::task::Poll::Ready(Ok((addr, buf.filled().len()))),
        }
    }
}

impl kcp_rust::AsyncRecv for TunnelSocket {
    fn poll_recv(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        let mut buf = ReadBuf::new(buf);
        match self.0.poll_recv(cx, &mut buf)? {
            std::task::Poll::Pending => std::task::Poll::Pending,
            std::task::Poll::Ready(()) => std::task::Poll::Ready(Ok(buf.filled().len())),
        }
    }
}

impl kcp_rust::AsyncSendTo for TunnelSocket {
    fn poll_sendto(
        &mut self,
        cx: &mut std::task::Context<'_>,
        addr: &std::net::SocketAddr,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        self.0.poll_send_to(cx, buf, addr.clone())
    }
}

impl kcp_rust::AsyncSend for TunnelSocket {
    fn poll_send(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        self.0.poll_send(cx, buf)
    }
}

impl kcp_rust::KcpRuntime for KcpRuntimeWithTokio {
    type Err = std::io::Error;
    type Runner = KcpRunnerWithTokio;

    type Timer = KcpTimerWithTokio;

    fn timer() -> Self::Timer {
        KcpTimerWithTokio
    }
}

impl kcp_rust::Runner for KcpRunnerWithTokio {
    type Err = std::io::Error;

    fn call(process: kcp_rust::Processor) -> std::result::Result<(), Self::Err> {
        std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(process)
        });
        Ok(())
    }
}

impl kcp_rust::Timer for KcpTimerWithTokio {
    type Ret = ();
    type Output = BoxedFuture<()>;

    fn sleep(&self, time: std::time::Duration) -> Self::Output {
        Box::pin(async move { tokio::time::sleep(time).await })
    }
}

impl<K> tokio::io::AsyncRead for KcpTunnelStream<K>
where
    K: kcp_rust::AsyncRead + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match kcp_rust::AsyncRead::poll_read(Pin::new(&mut self.0), cx, buf.initialized_mut())? {
            std::task::Poll::Pending => std::task::Poll::Pending,
            std::task::Poll::Ready(n) => {
                buf.advance(n);
                std::task::Poll::Ready(Ok(()))
            }
        }
    }
}

impl<K> tokio::io::AsyncWrite for KcpTunnelStream<K>
where
    K: kcp_rust::AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.0).poll_close(cx)
    }
}
