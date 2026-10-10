//! Why a call produced no answer, and the exit code each reason owns.
//!
//! Exit codes: 2 the call is wrong (usage, InvalidInput, TooLarge,
//! Misconfigured); 3 Unavailable (retry once); 4 no answer and a retry cannot
//! help (Rejected, Unencodable, Internal); 5 ReadBackDiffers. Exit 0 and 1 are
//! answers (the mind said yes, the mind said no) and are not troubles.

use std::fmt;

use eureka_state::ClientError;

#[derive(Debug)]
pub enum Trouble {
    Misconfigured(String),
    InvalidInput(String),
    TooLarge { bytes: usize, limit: usize },
    Unavailable(String),
    Rejected(String),
    Unencodable(String),
    Internal(String),
    ReadBackDiffers(String),
}

impl Trouble {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Misconfigured(_) => "Misconfigured",
            Self::InvalidInput(_) => "InvalidInput",
            Self::TooLarge { .. } => "TooLarge",
            Self::Unavailable(_) => "Unavailable",
            Self::Rejected(_) => "Rejected",
            Self::Unencodable(_) => "Unencodable",
            Self::Internal(_) => "Internal",
            Self::ReadBackDiffers(_) => "ReadBackDiffers",
        }
    }

    pub fn exit(&self) -> u8 {
        match self {
            Self::Misconfigured(_) | Self::InvalidInput(_) | Self::TooLarge { .. } => 2,
            Self::Unavailable(_) => 3,
            Self::Rejected(_) | Self::Unencodable(_) | Self::Internal(_) => 4,
            Self::ReadBackDiffers(_) => 5,
        }
    }

    pub fn invalid(detail: impl Into<String>) -> Self {
        Self::InvalidInput(detail.into())
    }
}

impl fmt::Display for Trouble {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: ", self.code())?;
        match self {
            Self::Misconfigured(detail)
            | Self::InvalidInput(detail)
            | Self::Unavailable(detail)
            | Self::Rejected(detail)
            | Self::Unencodable(detail)
            | Self::Internal(detail)
            | Self::ReadBackDiffers(detail) => f.write_str(detail),
            Self::TooLarge { bytes, limit } => {
                write!(f, "the request encodes to {bytes} bytes and one send carries at most {limit}")
            }
        }
    }
}

impl From<ClientError> for Trouble {
    fn from(error: ClientError) -> Self {
        match error {
            ClientError::Unavailable { endpoint, detail } => Self::Unavailable(format!("rudp://{endpoint}: {detail}")),
            ClientError::Rejected { endpoint, code, detail } => Self::Rejected(format!("rudp://{endpoint}: {code}: {detail}")),
            ClientError::TooLarge { bytes, limit } => Self::TooLarge { bytes, limit },
            ClientError::Unencodable { detail } => Self::Unencodable(detail),
        }
    }
}
