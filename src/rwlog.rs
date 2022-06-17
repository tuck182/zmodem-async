use std::io::Error;
use std::pin::Pin;
use std::task::{Context, Poll};
use log::LogLevel::Debug;
use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite, BufReader, ReadBuf};
use pin_project_lite::pin_project;
use pretty_hex::pretty_hex;

pin_project! {
    pub struct ReadWriteLog<RW> {
        #[pin]
        inner: BufReader<RW>,
    }
}

impl<RW: AsyncRead + AsyncWrite> ReadWriteLog<RW> {
    pub fn new(rw: RW) -> ReadWriteLog<RW> {
        ReadWriteLog {
            inner: BufReader::new(rw),
        }
    }

    pub fn into_inner(self) -> RW {
        self.inner.into_inner()
    }
}

impl<R: AsyncRead> AsyncRead for ReadWriteLog<R> {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        match self.project().inner.poll_read(cx, buf) {
            Poll::Ready(Ok(r)) => {
                if log_enabled!(Debug) {
                    debug!("In:");
                    debug!("{}", pretty_hex(&buf.filled()));
                }
                Poll::Ready(Ok(r))
            },
            otherwise => otherwise,
        }
    }
}

impl<R: AsyncRead> AsyncBufRead for ReadWriteLog<R> {

    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<&[u8]>> {
        match self.project().inner.poll_fill_buf(cx) {
            Poll::Ready(Ok(r)) => {
                if log_enabled!(Debug) {
                    debug!("In:");
                    debug!("{}", pretty_hex(&r));
                }
                Poll::Ready(Ok(r))
            },
            otherwise => otherwise,
        }
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        self.project().inner.consume(amt)
    }
}

impl<RW: AsyncWrite + AsyncRead> AsyncWrite for ReadWriteLog<RW> {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, Error>> {
        match self.project().inner.poll_write(cx, buf) {
            Poll::Ready(Ok(n)) => {
                if log_enabled!(Debug) {
                    debug!("Out:");
                    debug!("{}", pretty_hex(&&buf[0..n]));
                }
                Poll::Ready(Ok(n))
            },
            otherwise => otherwise,
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        self.project().inner.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        self.project().inner.poll_shutdown(cx)
    }
}

