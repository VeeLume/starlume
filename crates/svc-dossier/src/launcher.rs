//! The RSI launcher's game library: which channels this account may play,
//! with their current builds (`sc_dossier::auth::library`). Account-scoped:
//! a channel missing from it is not open to this account right now, which
//! is not the same as closed.
//!
//! **Gated:** the caller must have passed `require_unofficial("launcher")`.
//! Two launcher API calls with the launcher's stored session; nothing is
//! cached here (the shell keeps the answer for an hour).

pub use sc_dossier::auth::{Game, GameChannel};

/// The account's game library. A stale or absent launcher session surfaces
/// as `sc_dossier::Error::Mint`.
pub async fn library() -> anyhow::Result<Vec<Game>> {
    let creds = tokio::task::spawn_blocking(sc_dossier::store::read_credentials).await??;
    let opts = sc_dossier::auth::MintOptions::from_credentials(&creds);
    Ok(sc_dossier::auth::library(&creds, &opts).await?)
}
