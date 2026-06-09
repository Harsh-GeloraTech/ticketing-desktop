// Errors raised by the printer service. These are converted into friendly
// `PrintResult` / status messages before crossing into the UI.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PrinterError {
    #[error("printer discovery failed: {0}")]
    Discovery(String),

    #[error("printer '{0}' was not found")]
    NotFound(String),

    #[error("printer '{0}' is disconnected or offline")]
    Offline(String),

    #[error("print job failed: {0}")]
    Print(String),

    #[error("print timed out")]
    Timeout,

    #[error("database error: {0}")]
    Db(String),
}

impl From<sqlx::Error> for PrinterError {
    fn from(e: sqlx::Error) -> Self {
        PrinterError::Db(e.to_string())
    }
}
