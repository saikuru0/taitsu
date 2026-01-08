use crate::panels;
use crate::state::{ChatSettings, ConnectionCache, Panels, UiCache};
use crate::utils::load_accounts;
use eframe::egui;
use oshatori::{
    client::StateClient,
    connection::{Connection, ConnectionEvent, MockConnection, SockchatConnection},
    AuthField, Profile, Protocol,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
use tokio::sync::Mutex as TokioMutex;

pub type DynConnection = Arc<TokioMutex<Box<dyn Connection>>>;

pub struct ChatClient {
    pub state_client: Arc<StateClient>,
    pub cache: Arc<Mutex<UiCache>>,
    pub runtime: Arc<Runtime>,
    pub connections: Arc<Mutex<HashMap<String, DynConnection>>>,
    pub account_to_conn: Arc<Mutex<HashMap<usize, String>>>,

    pub new_message: String,
    pub show_account_popup: bool,
    pub temp_auth: Vec<AuthField>,
    pub editing_account: Option<usize>,
    pub temp_profile: Profile,
    pub selected_account: Option<usize>,
    pub selected_protocol: Option<usize>,
    pub protocols: Vec<Protocol>,
    pub panels: Panels,
    pub chat_settings: ChatSettings,
    pub show_asset_picker: bool,

    pub update_interval: Duration,
}

impl ChatClient {
    pub fn new(runtime: Arc<Runtime>) -> Self {
        let client = Self {
            state_client: Arc::new(StateClient::new()),
            cache: Arc::new(Mutex::new(UiCache::default())),
            runtime,
            connections: Arc::new(Mutex::new(HashMap::new())),
            account_to_conn: Arc::new(Mutex::new(HashMap::new())),
            new_message: String::new(),
            show_account_popup: false,
            temp_auth: Vec::new(),
            editing_account: None,
            temp_profile: Profile::default(),
            selected_account: None,
            selected_protocol: None,
            protocols: available_protocols(),
            panels: Panels {
                accounts: true,
                channels: true,
                users: true,
                chat: true,
                input: true,
                settings: false,
            },
            chat_settings: ChatSettings::default(),
            show_asset_picker: false,
            update_interval: Duration::from_millis(500),
        };

        {
            let mut cache = client.cache.lock().unwrap();
            cache.accounts = load_accounts();
        }

        client.start_updates();
        client.auto_connect_accounts();
        client
    }

    pub fn name() -> &'static str {
        "Taitsu"
    }

    fn start_updates(&self) {
        let state_client = self.state_client.clone();
        let cache = self.cache.clone();
        let account_to_conn = self.account_to_conn.clone();

        self.runtime.spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(500));

            loop {
                interval.tick().await;

                let conn_ids = state_client.list_connections().await;
                let mut connection_caches: HashMap<String, ConnectionCache> = HashMap::new();

                let acc_to_conn = account_to_conn.lock().unwrap().clone();
                let conn_to_acc: HashMap<String, usize> =
                    acc_to_conn.iter().map(|(k, v)| (v.clone(), *k)).collect();

                for conn_id in &conn_ids {
                    let account_index = conn_to_acc.get(conn_id).copied().unwrap_or(0);

                    if let Some(state) = state_client.get_connection(conn_id).await {
                        let channels: Vec<String> = state.channels.keys().cloned().collect();
                        let mut current_channel = None;
                        let mut assets = HashMap::new();
                        let mut global_users = HashMap::new();

                        if let Some(ch_id) = &state.current_channel {
                            if let Some(ch) = state_client.get_channel(conn_id, ch_id).await {
                                for (id, asset) in &ch.assets {
                                    assets.insert(id.clone(), asset.clone());
                                }
                                current_channel = Some(ch);
                            }
                        }

                        for (id, asset) in &state.global_assets {
                            assets.insert(id.clone(), asset.clone());
                        }

                        for (id, user) in &state.global_users {
                            global_users.insert(id.clone(), user.clone());
                        }

                        let current_user = if let Some(uid) = &state.current_user_id {
                             state_client.get_user(conn_id, uid).await
                        } else {
                             None
                        };

                        let conn_cache = ConnectionCache {
                            connection_id: conn_id.clone(),
                            account_index,
                            status: state.status.clone(),
                            channels,
                            current_channel,
                            assets,
                            global_users,
                            current_user,
                            pending_messages: Vec::new(),
                            last_ping: None,
                        };

                        connection_caches.insert(conn_id.clone(), conn_cache);
                    }
                }

                if let Ok(mut c) = cache.lock() {
                    let active = c.active_connection.clone();

                    for (conn_id, new_cache) in connection_caches {
                        if let Some(existing) = c.connections.get_mut(&conn_id) {
                            if let Some(ref ch) = new_cache.current_channel {
                                existing.pending_messages.retain(|pending| {
                                    !ch.messages.iter().any(|m| {
                                        m.sender_id == pending.sender_id
                                            && m.content == pending.content
                                    })
                                });
                            }

                            existing.status = new_cache.status;
                            existing.channels = new_cache.channels;
                            existing.current_channel = new_cache.current_channel;
                            existing.assets = new_cache.assets;
                            existing.global_users = new_cache.global_users;
                            existing.current_user = new_cache.current_user;
                        } else {
                            c.connections.insert(conn_id, new_cache);
                        }
                    }

                    c.connections.retain(|id, _| conn_ids.contains(id));

                    if active.is_none() && !c.connections.is_empty() {
                        c.active_connection = c.connections.keys().next().cloned();
                    }

                    c.updated = Some(Instant::now());
                }
            }
        });
    }

    pub fn active_connection(&self) -> Option<ConnectionCache> {
        let cache = self.cache.lock().unwrap();
        cache
            .active_connection
            .as_ref()
            .and_then(|id| cache.connections.get(id))
            .cloned()
    }

    pub fn set_active_connection(&self, conn_id: String) {
        let mut cache = self.cache.lock().unwrap();
        if cache.connections.contains_key(&conn_id) {
            cache.active_connection = Some(conn_id);
        }
    }

    pub fn auto_connect_accounts(&self) {
        let accounts: Vec<(usize, oshatori::Account)> = {
            let cache = self.cache.lock().unwrap();
            cache
                .accounts
                .iter()
                .enumerate()
                .filter(|(_, a)| a.autoconnect)
                .map(|(i, a)| (i, a.clone()))
                .collect()
        };

        for (idx, account) in accounts {
            self.connect_account(idx, account);
        }
    }

    pub fn connect_account(&self, account_idx: usize, account: oshatori::Account) {
        let state_client = self.state_client.clone();
        let connections = self.connections.clone();
        let cache = self.cache.clone();
        let account_to_conn = self.account_to_conn.clone();
        let auth = account.auth.clone();
        let protocol = account.protocol_name.clone();

        self.runtime.spawn(async move {
            let conn_id = state_client.track(&protocol).await;
            account_to_conn
                .lock()
                .unwrap()
                .insert(account_idx, conn_id.clone());

            let mut current_user_id = None;
            for field in &auth {
                if field.name == "uid" {
                    if let oshatori::FieldValue::Text(Some(v)) = &field.value {
                        current_user_id = Some(v.clone());
                    }
                }
            }

            {
                let mut cache = cache.lock().unwrap();
                let conn_cache = ConnectionCache {
                    connection_id: conn_id.clone(),
                    account_index: account_idx,
                    status: oshatori::client::ConnectionStatus::Connecting,
                    channels: Vec::new(),
                    current_channel: None,
                    assets: std::collections::HashMap::new(),
                    global_users: std::collections::HashMap::new(),
                    current_user: current_user_id.map(|id| oshatori::Profile {
                        id: Some(id),
                        username: None,
                        display_name: None,
                        color: None,
                        picture: None,
                    }),
                    pending_messages: Vec::new(),
                    last_ping: None,
                };
                cache.connections.insert(conn_id.clone(), conn_cache);
                if cache.active_connection.is_none() {
                    cache.active_connection = Some(conn_id.clone());
                }
            }

            match protocol.to_lowercase().as_str() {
                p if p.contains("mock") => {
                    use oshatori::connection::MockConnection;
                    let mut conn = MockConnection::new();
                    let _ = conn.set_auth(auth);
                    let rx = conn.subscribe();
                    state_client.spawn_processor(conn_id.clone(), rx);
                    if conn.connect().await.is_ok() {
                        let boxed: Box<dyn oshatori::Connection> = Box::new(conn);
                        connections
                            .lock()
                            .unwrap()
                            .insert(conn_id, std::sync::Arc::new(tokio::sync::Mutex::new(boxed)));
                    }
                }
                p if p.contains("sockchat") => {
                    use oshatori::connection::SockchatConnection;
                    let mut conn = SockchatConnection::new();
                    let _ = conn.set_auth(auth);
                    let rx = conn.subscribe();
                    state_client.spawn_processor(conn_id.clone(), rx);
                    if conn.connect().await.is_ok() {
                        let boxed: Box<dyn oshatori::Connection> = Box::new(conn);
                        connections
                            .lock()
                            .unwrap()
                            .insert(conn_id, std::sync::Arc::new(tokio::sync::Mutex::new(boxed)));
                    }
                }
                _ => {}
            }
        });
    }

    pub fn disconnect_account(&self, account_idx: usize) {
        let conn_id = {
            let acc_to_conn = self.account_to_conn.lock().unwrap();
            acc_to_conn.get(&account_idx).cloned()
        };

        if let Some(conn_id) = conn_id {
            let conn = {
                self.connections.lock().unwrap().get(&conn_id).cloned()
            };
            if let Some(conn) = conn {
                self.runtime.spawn(async move {
                    let mut conn = conn.lock().await;
                    let _ = conn.disconnect().await;
                });
            }

            self.account_to_conn.lock().unwrap().remove(&account_idx);
            self.connections.lock().unwrap().remove(&conn_id);

            let state_client = self.state_client.clone();
            let conn_id_clone = conn_id.clone();

            self.runtime.spawn(async move {
                state_client.untrack(&conn_id_clone).await;
            });

            let mut cache = self.cache.lock().unwrap();
            cache.connections.remove(&conn_id);

            if cache.active_connection.as_ref() == Some(&conn_id) {
                cache.active_connection = cache.connections.keys().next().cloned();
            }
        }
    }

    pub fn sync_selection(&self, channel_id: String) {
        let active = {
            let cache = self.cache.lock().unwrap();
            cache.active_connection.clone()
        };
        let state_client = self.state_client.clone();

        self.runtime.spawn(async move {
            if let Some(cid) = active {
                use oshatori::connection::ChannelEvent;
                state_client
                    .process(
                        &cid,
                        ConnectionEvent::Channel {
                            event: ChannelEvent::Switch { channel_id },
                        },
                    )
                    .await;
            }
        });
    }

    pub fn send_event(&self, event: ConnectionEvent) {
        let active = {
            let cache = self.cache.lock().unwrap();
            cache.active_connection.clone()
        };
        let connections = self.connections.clone();

        self.runtime.spawn(async move {
            if let Some(cid) = active {
                let conn = {
                    let conns = connections.lock().unwrap();
                    conns.get(&cid).cloned()
                };
                if let Some(conn) = conn {
                    let mut conn = conn.lock().await;
                    let _ = conn.send(event).await;
                }
            }
        });
    }

    pub fn add_pending_message(&self, message: oshatori::Message) {
        let mut cache = self.cache.lock().unwrap();
        let conn_id = cache.active_connection.clone();
        if let Some(conn_id) = conn_id {
            if let Some(conn_cache) = cache.connections.get_mut(&conn_id) {
                conn_cache.pending_messages.push(message);
            }
        }
    }

    fn menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.panels.accounts, "Accounts");
                    ui.checkbox(&mut self.panels.channels, "Channels");
                    ui.checkbox(&mut self.panels.users, "Users");
                    ui.checkbox(&mut self.panels.chat, "Chat");
                    ui.checkbox(&mut self.panels.input, "Input");
                    ui.checkbox(&mut self.panels.settings, "Settings");
                });

                ui.separator();
                ui.label("Taitsu");
            });
        });
    }
}

impl eframe::App for ChatClient {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.menu_bar(ctx);
        panels::draw_accounts(self, ctx);
        panels::draw_channels(self, ctx);
        panels::draw_users(self, ctx);
        panels::draw_settings(self, ctx);
        panels::draw_chat(self, ctx);
        panels::draw_popups(self, ctx);
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

fn available_protocols() -> Vec<Protocol> {
    vec![
        MockConnection::new().protocol_spec(),
        SockchatConnection::new().protocol_spec(),
    ]
}
