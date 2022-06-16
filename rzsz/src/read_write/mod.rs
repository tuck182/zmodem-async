use pin_project_lite::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::io;

pin_project! {
    pub struct AsyncReadWrite<R, W> {
        #[pin]
        inner_read:  R,
        #[pin]
        inner_write: W,
    }
}

impl<R, W> AsyncReadWrite<R, W>
    where R: AsyncRead + Unpin, W: AsyncWrite + Unpin {
    pub fn new(read: R, write: W) -> Self {
        Self {
            inner_read:  read,
            inner_write: write,
        }
    }
}

impl<R, W> AsyncRead for AsyncReadWrite<R, W>
    where R: AsyncRead + Unpin, W: AsyncWrite + Unpin {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut io::ReadBuf<'_>) -> Poll<io::Result<()>> {
        self.project().inner_read.poll_read(cx, buf)
    }
}

impl<R, W> AsyncWrite for AsyncReadWrite<R, W>
    where R: AsyncRead + Unpin, W: AsyncWrite + Unpin {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.project().inner_write.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.project().inner_write.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.project().inner_write.poll_shutdown(cx)
    }
}
