use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("This pdf is invalid: ${0}")]
    InvalidFile(&'static str),

    #[error("Token parsing error")]
    LexicalError,

    #[error("Object parsing error")]
    ParserError,

    #[error("This pdf is missing data: ${0}")]
    MissingDataException(&'static str),
}
