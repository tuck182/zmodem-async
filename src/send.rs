use std::io::SeekFrom;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt, AsyncWrite, AsyncWriteExt};

use crate::error::Result;
use crate::consts::*;
use crate::proto::*;
use crate::rwlog;
use crate::frame::*;

const SUBPACKET_SIZE: usize = 1024 * 8;
const SUBPACKET_PER_ACK: usize = 10;

#[derive(Debug, PartialEq)]
enum State {
    /// Waiting ZRINIT invite (do nothing)
    WaitingInit,

    /// Sending ZRQINIT
    SendingZRQINIT,

    /// Sending ZFILE frame
    SendingZFILE,

    /// Do nothing, just waiting for ZPOS
    WaitingZPOS,

    /// Sending ZDATA & subpackets
    SendingData,

    /// Sending ZFIN
    SendingZFIN,

    /// All works done, exiting
    Done,
}

impl State {
    fn new() -> State {
        State::WaitingInit
    }

    fn next(self, frame: &Frame) -> State {
        match (self, frame.get_frame_type()) {
            (State::WaitingInit,  ZRINIT)   => State::SendingZFILE,
            (State::WaitingInit,  _)        => State::SendingZRQINIT,

            (State::SendingZRQINIT, ZRINIT) => State::SendingZFILE,

            (State::SendingZFILE, ZRPOS)    => State::SendingData,
            (State::SendingZFILE, ZRINIT)   => State::WaitingZPOS,

            (State::WaitingZPOS, ZRPOS)     => State::SendingData,

            (State::SendingData,  ZACK)     => State::SendingData,
            (State::SendingData,  ZRPOS)    => State::SendingData,
            (State::SendingData,  ZRINIT)   => State::SendingZFIN,

            (State::SendingZFIN,  ZFIN)     => State::Done,

            (s, _) => {
               error!("Unexpected (state, frame) combination: {:#?} {}", s, frame);
               s // don't change current state
            },
        }
    }
}

pub async fn send<RW, R>(rw: RW, r: &mut R, filename: &str, filesize: Option<u32>) -> Result<RW>
    where RW: AsyncRead + AsyncWrite + Unpin,
          R:  AsyncRead + AsyncSeek + Unpin
{
    let mut rw_log = rwlog::ReadWriteLog::new(rw);

    let mut data = [0; SUBPACKET_SIZE];
    let mut offset: u32;

    write_zrqinit(&mut rw_log).await?;

    let mut state = State::new();

    while state != State::Done {
        rw_log.flush().await?;

        if !find_zpad(&mut rw_log).await? {
            continue;
        }

        let frame = match parse_header(&mut rw_log).await? {
            Some(x) => x,
            None    => { write_znak(&mut rw_log).await?; continue },
        };

        state = state.next(&frame);
        debug!("State: {:?}", state);

        // do things according new state
        match state {
            State::SendingZRQINIT => {
                write_zrqinit(&mut rw_log).await?;
            },
            State::SendingZFILE => {
                write_zfile(&mut rw_log, filename, filesize).await?;
            },
            State::SendingData  => {
                offset = frame.get_count();
                r.seek(SeekFrom::Start(offset as u64)).await?;

                let num = r.read(&mut data).await?;

                if num == 0 {
                    write_zeof(&mut rw_log, offset).await?;
                }
                else {
                    // ZBIN32|ZDATA
                    // ZCRCG - best perf
                    // ZCRCQ - mid perf
                    // ZCRCW - worst perf
                    // ZCRCE - send at end
                    write_zdata(&mut rw_log, offset).await?;

                    let mut i = 0;
                    loop {
                        i += 1;

                        write_zlde_data(&mut rw_log, ZCRCG, &data[..num]).await?;
                        offset += num as u32;

                        let num = r.read(&mut data).await?;
                        if num < data.len() || i >= SUBPACKET_PER_ACK {
                            write_zlde_data(&mut rw_log, ZCRCW, &data[..num]).await?;
                            break;
                        }
                    }
                }
            },
            State::SendingZFIN  => {
                write_zfin(&mut rw_log).await?;
            },
            State::Done         => {
                write_over_and_out(&mut rw_log).await?;
            },
            _ => (),
        }
    }

    Ok(rw_log.into_inner())
}

