use std::{thread, time};
use std::str::from_utf8;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::error::Result;
use crate::consts::*;
use crate::proto::*;
use crate::rwlog;
use crate::frame::*;

#[derive(Debug, PartialEq)]
enum State {
    /// Sending ZRINIT
    SendingZRINIT,

    /// Processing ZFILE supplementary data
    ProcessingZFILE,

    /// Receiving file's content
    ReceivingData,

    /// Checking length of received data
    CheckingData,

    /// All works done, exiting
    Done,
}

impl State {
    fn new() -> State {
        State::SendingZRINIT
    }

    fn next(self, frame: &Frame) -> State {
        match (self, frame.get_frame_type()) {
            (State::SendingZRINIT, ZFILE)   => State::ProcessingZFILE,
            (State::SendingZRINIT, _)       => State::SendingZRINIT,

            (State::ProcessingZFILE, ZDATA) => State::ReceivingData,
            (State::ProcessingZFILE, _)     => State::ProcessingZFILE,

            (State::ReceivingData, ZDATA)   => State::ReceivingData,
            (State::ReceivingData, ZEOF)    => State::CheckingData,

            (State::CheckingData, ZDATA)    => State::ReceivingData,
            (State::CheckingData, ZFIN)     => State::Done,

            (s, _) => {
               error!("Unexpected (state, frame) combination: {:#?} {}", s, frame);
               s // don't change current state
            },
        }
    }
}

/// Receives data by Z-Modem protocol
pub async fn recv<RW, W>(rw: RW, mut w: W) -> Result<usize>
    where RW: AsyncRead + AsyncWrite + Unpin,
          W:  AsyncWrite + Unpin
{
    let mut rw_log = rwlog::ReadWriteLog::new(rw);
    let mut count = 0;

    let mut state = State::new();

    write_zrinit(&mut rw_log).await?;

    while state != State::Done {
        if !find_zpad(&mut rw_log).await? {
            continue;
        }

        let frame = match parse_header(&mut rw_log).await? {
            Some(x) => x,
            None    => { recv_error(&mut rw_log, &state, count).await?; continue },
        };

        state = state.next(&frame);
        debug!("State: {:?}", state);

        // do things according new state
        match state {
            State::SendingZRINIT => {
                write_zrinit(&mut rw_log).await?;
            },
            State::ProcessingZFILE => {
                let mut buf = Vec::new();

                if recv_zlde_frame(frame.get_header(), &mut rw_log, &mut buf).await?.is_none() {
                    write_znak(&mut rw_log).await?;
                }
                else {
                    write_zrpos(&mut rw_log, count).await?;

                    // TODO: process supplied data
                    if let Ok(s) = from_utf8(&buf) {
                        debug!(target: "proto", "ZFILE supplied data: {}", s);
                    }
                }
            },
            State::ReceivingData => {
                if frame.get_count() != count ||
                    !recv_data(frame.get_header(), &mut count, &mut rw_log, &mut w).await? {
                    write_zrpos(&mut rw_log, count).await?;
                }
            },
            State::CheckingData => {
                if frame.get_count() != count {
                    error!("ZEOF offset mismatch: frame({}) != recv({})", frame.get_count(), count);
                    // receiver ignores the ZEOF because a new zdata is coming
                }
                else {
                    write_zrinit(&mut rw_log).await?;
                }
            },
            State::Done => {
                write_zfin(&mut rw_log).await?;
                thread::sleep(time::Duration::from_millis(10)); // sleep a bit
            },
        }
    }

    Ok(count as usize)
}

async fn recv_error<W>(w: &mut W, state: &State, count: u32) -> Result<()>
    where W: AsyncWrite + Unpin
{
    // TODO: flush input

    match *state {
        State::ReceivingData => write_zrpos(w, count).await,
        _                    => write_znak(w).await,
    }
}

