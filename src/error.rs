use std::fmt;

#[derive(Debug)]
pub enum AppError {
    InvalidPubkey(String),
    NoRelaysAvailable,
    NoEvents,
    Relay(nostr_sdk::client::Error),
    NostrKey(nostr_sdk::key::Error),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::InvalidPubkey(e) => write!(f, "Invalid public key: {}", e),
            AppError::NoRelaysAvailable => write!(f, "No relays could be added"),
            AppError::NoEvents => write!(f, "No events found for this node"),
            AppError::Relay(e) => write!(f, "Relay error: {}", e),
            AppError::NostrKey(e) => write!(f, "Nostr key error: {}", e),
        }
    }
}

impl std::error::Error for AppError {}

impl From<nostr_sdk::client::Error> for AppError {
    fn from(e: nostr_sdk::client::Error) -> Self {
        AppError::Relay(e)
    }
}

impl From<nostr_sdk::key::Error> for AppError {
    fn from(e: nostr_sdk::key::Error) -> Self {
        AppError::NostrKey(e)
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
