use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("This pdf is invalid: ${0}")]
    InvalidFile(&'static str),
}
