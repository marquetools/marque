use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("malformed marking: {0:?}")]
    MalformedMarking(String),

    #[error("unrecognized token at offset {offset}: {token:?}")]
    UnrecognizedToken { token: String, offset: usize },

    #[error("empty source buffer")]
    EmptySource,
}
