//! The playlist list and Home's "Made for you" as last seen, kept on disk so
//! they show the moment the app opens.
//!
//! Both come from the shared Web API app: Development Mode hides Spotify's
//! own playlists from a personal app. Every user of the shared app shares
//! its quota, and at busy times each request waits out a cooldown of about
//! thirty seconds. What was seen last stands in until the fresh answer
//! arrives and replaces it, as Liked Songs does (`crate::liked`).

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::api::models::Playlist;

const VERSION: u32 = 1;

/// One account's saved copy of something the shared app answers slowly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Saved<T> {
    version: u32,
    pub account_id: String,
    pub value: T,
}

impl<T> Saved<T> {
    pub fn new(account_id: &str, value: T) -> Self {
        Self {
            version: VERSION,
            account_id: account_id.to_string(),
            value,
        }
    }
}

/// The account's playlists, in library order.
pub type Playlists = Saved<Vec<Playlist>>;

/// "Made for you", by the search term each shelf comes from.
pub type MadeForYou = Saved<BTreeMap<String, Vec<Playlist>>>;

/// Both, as read at startup.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cache {
    pub playlists: Option<Vec<Playlist>>,
    pub made_for_you: Option<BTreeMap<String, Vec<Playlist>>>,
}

/// A saved copy for `account`, or nothing when it is missing, unreadable,
/// from another version or another account.
pub async fn read<T: serde::de::DeserializeOwned>(path: &Path, account: &str) -> Option<T> {
    let bytes = tokio::fs::read(path).await.ok()?;
    let saved: Saved<T> = serde_json::from_slice(&bytes).ok()?;
    (saved.version == VERSION && saved.account_id == account).then_some(saved.value)
}

/// Replaces the saved copy at `path` whole, so a crash never leaves half.
pub async fn write<T: Serialize>(path: &Path, saved: &Saved<T>) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let bytes = serde_json::to_vec(saved).map_err(std::io::Error::other)?;
    let temporary = path.with_extension("json.tmp");
    tokio::fs::write(&temporary, bytes).await?;
    crate::util::replace_file(&temporary, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playlist(id: &str) -> Playlist {
        Playlist {
            id: id.into(),
            name: format!("Playlist {id}"),
            uri: format!("spotify:playlist:{id}"),
            ..Playlist::default()
        }
    }

    #[tokio::test]
    async fn a_saved_list_reads_back_only_for_its_account() {
        let dir =
            std::env::temp_dir().join(format!("spotifast-library-cache-{}", std::process::id()));
        let path = dir.join("playlists.json");
        let saved = Playlists::new("alice", vec![playlist("a"), playlist("b")]);
        write(&path, &saved).await.unwrap();
        let read_back: Option<Vec<Playlist>> = read(&path, "alice").await;
        assert_eq!(read_back, Some(saved.value.clone()));
        let other: Option<Vec<Playlist>> = read(&path, "bob").await;
        assert_eq!(other, None, "another account never sees it");
        let missing: Option<Vec<Playlist>> = read(&dir.join("missing.json"), "alice").await;
        assert_eq!(missing, None);
        let _ = std::fs::remove_dir_all(dir);
    }
}
