use std::fmt::Debug;
use thiserror::Error;

pub type Result<T> = core::result::Result<T, ZmodemError>;

#[derive(Debug, Error)]
pub enum ZmodemError {
    #[error("I/O error: {0}")]
    IoError(std::io::Error),
    #[error("Protocol error: {0}")]
    ProtocolError(ProtocolError),
}

impl From<std::io::Error> for ZmodemError {
    fn from(e: std::io::Error) -> Self {
        ZmodemError::IoError(e)
    }
}

impl From<ProtocolError> for ZmodemError {
    fn from(e: ProtocolError) -> Self {
        ZmodemError::ProtocolError(e)
    }
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("Unexpected ZCRC byte: {0:02X}")]
    UnexpectedByteError(u8),
}
