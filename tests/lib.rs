use lazy_static::lazy_static;
use pin_project_lite::pin_project;
use std::io::Cursor;
use std::pin::Pin;
use std::process::Stdio;
use std::result;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::fs::{File, OpenOptions, remove_file};
use tokio::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::process::Command;
use tokio::time::sleep;

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

lazy_static! {
    static ref LOG_INIT: result::Result<(), log::SetLoggerError> = env_logger::init();
    static ref RND_VALUES: Vec<u8> = {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut buf = vec![0; 1024 * 1024 * 11];
        rng.fill_bytes(&mut buf);
        buf
    };
}

#[tokio::test]
#[cfg(unix)]
async fn recv_from_sz() {
    let _ = LOG_INIT.is_ok();

    let mut f = File::create("recv_from_sz").await.unwrap();
    f.write_all(&RND_VALUES).await.unwrap();

    let sz = Command::new("sz")
            .arg("recv_from_sz")
            .stdout(Stdio::piped())
            .stdin(Stdio::piped())
            .spawn()
            .expect("sz failed to run");

    let child_stdin = sz.stdin.unwrap();
    let child_stdout = sz.stdout.unwrap();
    let mut inout = AsyncReadWrite::new(child_stdout, child_stdin);

    let mut c = Cursor::new(Vec::new());
    zmodem::recv::recv(&mut inout, &mut c).await.unwrap();

    sleep(Duration::from_millis(300)).await;
    remove_file("recv_from_sz").await.unwrap();

    assert_eq!(RND_VALUES.clone(), c.into_inner());
}

#[tokio::test]
#[cfg(unix)]
async fn send_to_rz() {
    let _ = LOG_INIT.is_ok();

    let _ = remove_file("send_to_rz");

    let sz = Command::new("rz")
            .stdout(Stdio::piped())
            .stdin(Stdio::piped())
            .spawn()
            .expect("rz failed to run");

    let child_stdin = sz.stdin.unwrap();
    let child_stdout = sz.stdout.unwrap();
    let mut inout = AsyncReadWrite::new(child_stdout, child_stdin);

    let len = RND_VALUES.len() as u32;
    let copy = RND_VALUES.clone();
    let mut cur = Cursor::new(&copy);

    sleep(Duration::from_millis(300)).await;

    zmodem::send::send(&mut inout, &mut cur, "send_to_rz", Some(len)).await.unwrap();

    sleep(Duration::from_millis(300)).await;

    let mut f = File::open("send_to_rz").await.expect("open 'send_to_rz'");
    let mut received = Vec::new();
    f.read_to_end(&mut received).await.unwrap();
    remove_file("send_to_rz").await.unwrap();

    assert_eq!(copy, received);
}

#[tokio::test]
#[cfg(unix)]
async fn lib_send_recv() {
    let _ = LOG_INIT;

    let _ = remove_file("test-fifo1");
    let _ = remove_file("test-fifo2");

    let _ = Command::new("mkfifo")
            .arg("test-fifo1")
            .spawn()
            .expect("mkfifo failed to run")
            .wait();

    let _ = Command::new("mkfifo")
            .arg("test-fifo2")
            .spawn()
            .expect("mkfifo failed to run")
            .wait();

    sleep(Duration::from_millis(300)).await;

    tokio::spawn(async move {
        let outf = OpenOptions::new().write(true).open("test-fifo1").await.unwrap();
        let inf = File::open("test-fifo2").await.unwrap();
        let mut inout = AsyncReadWrite::new(inf, outf);

        let origin = RND_VALUES.clone();
        let mut c = Cursor::new(&origin);

        zmodem::send::send(&mut inout, &mut c, "test", None).await.unwrap();
    });

    let mut c = Cursor::new(Vec::new());

    let inf = File::open("test-fifo1").await.unwrap();
    let outf = OpenOptions::new().write(true).open("test-fifo2").await.unwrap();
    let mut inout = AsyncReadWrite::new(inf, outf);

    zmodem::recv::recv(&mut inout, &mut c).await.unwrap();

    let _ = remove_file("test-fifo1").await;
    let _ = remove_file("test-fifo2").await;

    assert_eq!(RND_VALUES.clone(), c.into_inner());
}
