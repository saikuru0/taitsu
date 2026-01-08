use oshatori::client::ConnectionStatus;
use oshatori::{client::ChannelState, Account, Asset, Message, Profile};
use std::collections::HashMap;
use std::time::Instant;

#[derive(Clone, Default)]
pub struct ConnectionCache {
    pub connection_id: String,
    pub account_index: usize,
    pub status: ConnectionStatus,
    pub channels: Vec<String>,
    pub current_channel: Option<ChannelState>,
    pub assets: HashMap<String, Asset>,
    pub global_users: HashMap<String, Profile>,
    pub current_user: Option<Profile>,
    pub pending_messages: Vec<Message>,
    pub last_ping: Option<Instant>,
}

#[derive(Clone, Default)]
pub struct UiCache {
    pub accounts: Vec<Account>,
    pub connections: HashMap<String, ConnectionCache>,
    pub active_connection: Option<String>,
    pub updated: Option<Instant>,
}

#[derive(Default)]
pub struct Panels {
    pub accounts: bool,
    pub channels: bool,
    pub users: bool,
    pub chat: bool,
    pub input: bool,
    pub settings: bool,
}

#[derive(Clone)]
pub struct ChatSettings {
    pub autoscroll: bool,
    pub auto_embed_images: bool,
    pub auto_embed_videos: bool,
    pub auto_embed_audio: bool,
    pub auto_embed_emotes: bool,
    pub auto_embed_stickers: bool,
    pub last_message_count: usize,
    pub embed_generation: usize,
    pub unembed_override: bool,
}

impl Default for ChatSettings {
    fn default() -> Self {
        ChatSettings {
            autoscroll: true,
            auto_embed_images: true,
            auto_embed_videos: false,
            auto_embed_audio: false,
            auto_embed_emotes: true,
            auto_embed_stickers: true,
            last_message_count: 0,
            embed_generation: 0,
            unembed_override: false,
        }
    }
}
