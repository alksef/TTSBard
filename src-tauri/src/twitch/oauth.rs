//! Shared roles and errors for Twitch Device Code authorization.

use twitch_oauth2::Scope;

use super::api::ApiError;

/// The two authorization roles with their exact minimal scopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuthRole {
    Bot,
    Broadcaster,
}

impl OAuthRole {
    pub fn scopes(&self) -> &'static [Scope] {
        match self {
            OAuthRole::Bot => &[Scope::UserWriteChat, Scope::UserBot],
            OAuthRole::Broadcaster => &[Scope::ChannelBot],
        }
    }
}

/// Stable, non-secret OAuth error codes.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OAuthError {
    #[error("twitch.oauth.cancelled")]
    Cancelled,
    #[error("twitch.oauth.timed_out")]
    TimedOut,
    #[error("twitch.oauth.denied")]
    Denied,
    #[error("twitch.oauth.malformed")]
    Malformed,
    #[error("twitch.oauth.exchange_failed")]
    ExchangeFailed(#[from] ApiError),
}
