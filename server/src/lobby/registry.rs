//! The room registry: a concurrent map from invite code to the room's command
//! channel, with collision-safe room creation.

use std::collections::HashSet;
use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::mpsc;

use boiling_point_protocol::RoomCode;

use crate::config::ContentConfig;
use crate::content::ContentRegistry;

use super::codes::generate_code;
use super::room::{spawn, RoomCommand};

/// Concurrent registry of live rooms. Holds the shared content the rooms need to
/// run games.
pub struct RoomRegistry {
    rooms: DashMap<RoomCode, mpsc::Sender<RoomCommand>>,
    registry: Arc<ContentRegistry>,
    config: Arc<ContentConfig>,
    emote_palette: Arc<HashSet<u16>>,
}

impl RoomRegistry {
    /// Create an empty registry sharing `registry`/`config` with every room it
    /// spawns; the emote palette is derived from the config's enabled emotes.
    pub fn new(registry: Arc<ContentRegistry>, config: Arc<ContentConfig>) -> Self {
        let emote_palette = Arc::new(
            config
                .emote
                .iter()
                .filter(|e| e.enabled)
                .map(|e| e.id)
                .collect::<HashSet<u16>>(),
        );
        RoomRegistry {
            rooms: DashMap::new(),
            registry,
            config,
            emote_palette,
        }
    }

    /// Create a fresh room with a unique invite code; returns the code and its
    /// command channel.
    pub fn create(&self) -> (RoomCode, mpsc::Sender<RoomCommand>) {
        loop {
            let code = generate_code();
            if !self.rooms.contains_key(&code) {
                let handle = spawn(
                    code.clone(),
                    self.registry.clone(),
                    self.config.clone(),
                    self.emote_palette.clone(),
                );
                self.rooms.insert(code.clone(), handle.tx.clone());
                return (code, handle.tx);
            }
        }
    }

    /// Look up an existing room's command channel by code.
    pub fn get(&self, code: &RoomCode) -> Option<mpsc::Sender<RoomCommand>> {
        self.rooms.get(code).map(|r| r.clone())
    }

    /// Number of live rooms (for metrics/tests).
    pub fn len(&self) -> usize {
        self.rooms.len()
    }

    /// Whether there are no live rooms.
    pub fn is_empty(&self) -> bool {
        self.rooms.is_empty()
    }
}
