use chrono::Utc;
use eframe::egui;
use eframe::egui::{Align2, Color32};
use egui::{Image, Ui};
use oshatori::connection::{
    ChannelEvent, ChatEvent, Connection, ConnectionEvent, SockchatConnection, UserEvent,
};
use oshatori::{
    Account, AuthField, FieldValue, Message, MessageFragment, MessageStatus, MessageType, Profile,
    Protocol,
};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::{broadcast, Mutex};

fn color_to_color32(opt: Option<[u8; 4]>) -> Color32 {
    opt.map(|rgb| Color32::from_rgba_unmultiplied(rgb[0], rgb[1], rgb[2], rgb[3]))
        .unwrap_or(Color32::WHITE)
}

pub enum ConnectionWrapper {
    Sockchat(oshatori::connection::SockchatConnection),
    Mock(oshatori::connection::MockConnection),
}

impl Clone for ConnectionWrapper {
    fn clone(&self) -> Self {
        match self {
            ConnectionWrapper::Sockchat(conn) => ConnectionWrapper::Sockchat(conn.clone()),
            ConnectionWrapper::Mock(conn) => ConnectionWrapper::Mock(conn.clone()),
        }
    }
}

#[async_trait::async_trait]
impl Connection for ConnectionWrapper {
    async fn connect(&mut self, auth: Vec<AuthField>) -> Result<(), String> {
        match self {
            ConnectionWrapper::Sockchat(conn) => conn.connect(auth).await,
            ConnectionWrapper::Mock(conn) => conn.connect(auth).await,
        }
    }

    async fn disconnect(&mut self) -> Result<(), String> {
        match self {
            ConnectionWrapper::Sockchat(conn) => conn.disconnect().await,
            ConnectionWrapper::Mock(conn) => conn.disconnect().await,
        }
    }

    async fn send(&mut self, event: ConnectionEvent) -> Result<(), String> {
        match self {
            ConnectionWrapper::Sockchat(conn) => conn.send(event).await,
            ConnectionWrapper::Mock(conn) => conn.send(event).await,
        }
    }

    fn subscribe(&self) -> broadcast::Receiver<ConnectionEvent> {
        match self {
            ConnectionWrapper::Sockchat(conn) => conn.subscribe(),
            ConnectionWrapper::Mock(conn) => conn.subscribe(),
        }
    }

    fn protocol_spec() -> Protocol {
        unimplemented!("Use protocol from inner connection implementation")
    }
}

#[derive(Clone)]
pub struct ChatEventWrapper {
    pub channel_id: Option<String>,
    pub timestamp: chrono::DateTime<Utc>,
    pub sender: String,
    pub message_preview: Vec<MessageFragment>,
    pub profile: Option<Profile>,
}

fn prev_to_display(ui: &mut Ui, prev: Vec<MessageFragment>) {
    for frag in prev {
        match frag {
            MessageFragment::Text(text) => {
                ui.label(text);
            }
            MessageFragment::Image { url, .. } => {
                ui.add(
                    Image::from_uri(std::borrow::Cow::Owned(url))
                        .max_size(egui::Vec2 { x: 400.0, y: 600.0 })
                        .fit_to_original_size(1.0),
                );
            }
            MessageFragment::Video { url, .. } => {
                ui.hyperlink(url);
            }
            MessageFragment::Audio { url, .. } => {
                ui.hyperlink(url);
            }
            MessageFragment::Url(url) => {
                ui.hyperlink(url);
            }
        }
    }
}

pub struct ChatClient {
    pub accounts: Vec<Account>,
    pub connections: Vec<Arc<Mutex<Box<ConnectionWrapper>>>>,
    pub current_connection: Option<usize>,
    pub new_message: String,
    pub show_account_popup: bool,
    pub show_connection_popup: bool,
    pub temp_auth: Vec<AuthField>,
    pub editing_account: Option<usize>,
    pub temp_profile: Profile,
    pub selected_account: Option<usize>,
    pub selected_protocol: Option<usize>,
    pub protocols: Vec<Protocol>,
    pub runtime: Arc<Runtime>,
    pub chat_log: Arc<Mutex<Vec<ChatEventWrapper>>>,
    pub channels: Arc<Mutex<Vec<String>>>,
    pub current_channel: Arc<Mutex<Option<String>>>,
    pub online_users: Arc<Mutex<Vec<Profile>>>,
}

impl ChatClient {
    pub fn new_with_runtime(runtime: Arc<Runtime>) -> Self {
        let sockchat_protocol = SockchatConnection::protocol_spec();
        let mut me = Self {
            runtime,
            accounts: Vec::new(),
            connections: Vec::new(),
            current_connection: None,
            new_message: String::new(),
            show_account_popup: false,
            show_connection_popup: false,
            temp_auth: Vec::new(),
            editing_account: None,
            temp_profile: Profile {
                id: None,
                username: None,
                display_name: None,
                color: None,
                picture: None,
            },
            selected_account: None,
            selected_protocol: None,
            protocols: vec![sockchat_protocol],
            chat_log: Arc::new(Mutex::new(Vec::new())),
            channels: Arc::new(Mutex::new(Vec::new())),
            current_channel: Arc::new(Mutex::new(None)),
            online_users: Arc::new(Mutex::new(Vec::new())),
        };

        me.accounts = me.load_accounts();
        me
    }

    fn accounts_file_path() -> PathBuf {
        let mut path = std::env::current_dir().unwrap_or_default();
        path.push("accounts.json");
        path
    }

    fn load_accounts(&self) -> Vec<Account> {
        let path = Self::accounts_file_path();
        match fs::read_to_string(&path) {
            Ok(json) => serde_json::from_str(&json).unwrap_or_else(|_| Vec::new()),
            Err(_) => Vec::new(),
        }
    }

    fn save_accounts(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.accounts) {
            let _ = fs::write(Self::accounts_file_path(), json);
        }
    }

    pub fn name() -> &'static str {
        "Taitsu"
    }

    fn draw_top_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.heading(Self::name());
        });
    }

    fn draw_account_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("account_management").show(ctx, |ui| {
            ui.heading("Accounts");
            if ui.button("Add account").clicked() {
                self.editing_account = None;
                self.selected_protocol = None;
                self.temp_auth.clear();
                self.temp_profile = Profile {
                    id: None,
                    username: None,
                    display_name: None,
                    color: None,
                    picture: None,
                };
                self.show_account_popup = true;
            }
            ui.separator();
            ui.label("Available accounts:");
            for (index, account) in self.accounts.iter().enumerate() {
                let username = account
                    .private_profile
                    .as_ref()
                    .and_then(|p| p.username.clone())
                    .unwrap_or_else(|| "[no username]".to_string());
                if ui
                    .selectable_value(&mut self.selected_account, Some(index), username.clone())
                    .clicked()
                {
                    self.selected_account = Some(index);
                }
            }
            if let Some(idx) = self.selected_account {
                ui.horizontal(|ui| {
                    if ui.button("Edit").clicked() {
                        let account = &self.accounts[idx];
                        self.editing_account = Some(idx);
                        self.selected_protocol = self
                            .protocols
                            .iter()
                            .position(|p| p.name == account.protocol_name);
                        self.temp_auth = account.auth.clone();
                        self.temp_profile = account.private_profile.clone().unwrap_or(Profile {
                            id: None,
                            username: None,
                            display_name: None,
                            color: None,
                            picture: None,
                        });
                        self.show_account_popup = true;
                    }
                    if ui.button("Delete").clicked() {
                        self.accounts.remove(idx);
                        self.save_accounts();
                        self.selected_account = None;
                    }
                });
            }
        });
    }

    fn draw_channel_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("channel_panel").show(ctx, |ui| {
            ui.heading("Channels");
            let channels = self.channels.blocking_lock().clone();
            for ch in channels {
                if ui
                    .selectable_value(
                        &mut *self.current_channel.blocking_lock(),
                        Some(ch.clone()),
                        ch.clone(),
                    )
                    .clicked()
                {}
            }
        });
    }

    fn draw_online_users_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("online_users_panel").show(ctx, |ui| {
            ui.heading("Online users");
            let online = self.online_users.blocking_lock().clone();
            for user in online {
                let display = user
                    .display_name
                    .or(user.username)
                    .unwrap_or_else(|| "Unknown".to_owned());
                let color = color_to_color32(user.color);
                ui.horizontal(|ui| {
                    if let Some(pfp) = user.picture {
                        ui.add(
                            Image::from_uri(pfp).fit_to_exact_size(egui::Vec2 { x: 16.0, y: 16.0 }),
                        );
                    }
                    ui.colored_label(color, display);
                });
            }
        });
    }

    fn draw_chat_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.connections.is_empty() {
                ui.horizontal(|ui| {
                    ui.label("No active connections.");
                    if ui.button("+ Add connection").clicked() {
                        self.show_connection_popup = true;
                    }
                });
            } else {
                ui.horizontal(|ui| {
                    for (index, _) in self.connections.iter().enumerate() {
                        if ui
                            .selectable_label(
                                self.current_connection == Some(index),
                                format!("Conn {}", index + 1),
                            )
                            .clicked()
                        {
                            self.current_connection = Some(index);
                        }
                    }
                    if ui.button("+").clicked() {
                        self.show_connection_popup = true;
                    }
                });
                ui.separator();

                let active_channel = self.current_channel.blocking_lock().clone();

                let chat_events = self.chat_log.blocking_lock().clone();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for event in chat_events.iter() {
                            if let Some(active) = active_channel.clone() {
                                if let Some(ref ch) = event.channel_id {
                                    if ch != &active {
                                        continue;
                                    }
                                }
                            }
                            ui.horizontal(|ui| {
                                let sender = if let Some(profile) = &event.profile {
                                    profile.display_name.clone().unwrap_or_else(|| {
                                        profile.username.clone().unwrap_or(event.sender.clone())
                                    })
                                } else {
                                    event.sender.clone()
                                };

                                if let Some(profile) = &event.profile {
                                    if let Some(pfp) = &profile.picture {
                                        ui.add(
                                            Image::from_uri(pfp)
                                                .fit_to_exact_size(egui::Vec2 { x: 32.0, y: 32.0 }),
                                        );
                                    }
                                }

                                if let Some(profile) = &event.profile {
                                    ui.vertical(|ui| {
                                        ui.horizontal(|ui| {
                                            ui.colored_label(
                                                color_to_color32(profile.color),
                                                &sender,
                                            );

                                            ui.label(format!(
                                                "{}",
                                                event.timestamp.format("@ %H:%M:%S")
                                            ));
                                        });
                                        prev_to_display(ui, event.message_preview.clone());
                                    });
                                } else {
                                    ui.horizontal(|ui| {
                                        prev_to_display(ui, event.message_preview.clone());
                                        ui.label(format!(
                                            "{}",
                                            event.timestamp.format("@ %H:%M:%S")
                                        ));
                                    });
                                }
                            });
                            ui.separator();
                        }
                    });
            }
        });
    }

    fn draw_input_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("input_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let text_box_width = ui.available_width() - 80.0;
                ui.add_sized(
                    [text_box_width, 30.0],
                    egui::TextEdit::multiline(&mut self.new_message)
                        .hint_text("Type your message...")
                        .lock_focus(true),
                );

                let send_clicked = ui.button("Send").clicked()
                    || ui.input(|i| {
                        i.modifiers == egui::Modifiers::NONE && i.key_down(egui::Key::Enter)
                    });

                if send_clicked {
                    if let Some(current) = self.current_connection {
                        if !self.new_message.is_empty() {
                            let message_text = self.new_message.clone();
                            let conn_arc = self.connections[current].clone();
                            let runtime = self.runtime.clone();

                            let message = Message {
                                id: None,
                                sender_id: if let Some(snd_id) = self
                                    .accounts
                                    .get(self.selected_account.clone().unwrap())
                                    .as_ref()
                                    .unwrap()
                                    .private_profile
                                    .clone()
                                    .unwrap()
                                    .id
                                {
                                    Some(snd_id)
                                } else {
                                    None
                                },
                                content: vec![MessageFragment::Text(message_text.clone())],
                                timestamp: Utc::now(),
                                message_type: MessageType::CurrentUser,
                                status: MessageStatus::Sent,
                            };

                            runtime.spawn(async move {
                                let mut conn = conn_arc.lock().await;
                                if let Err(e) = conn
                                    .send(ConnectionEvent::Chat {
                                        event: ChatEvent::New {
                                            channel_id: None,
                                            message,
                                        },
                                    })
                                    .await
                                {
                                    eprintln!("Error sending message: {}", e);
                                }
                            });
                            self.new_message.clear();
                        }
                    }
                    ctx.request_repaint();
                }
            });
        });
    }

    fn generate_auth_ui(ui: &mut egui::Ui, fields: &mut Vec<AuthField>) {
        for field in fields.iter_mut() {
            ui.label(field.display.clone().unwrap_or(field.name.clone()));
            match &mut field.value {
                FieldValue::Text(ref mut val) => {
                    let mut current_text = val.clone().unwrap_or_default();
                    let response = ui.text_edit_singleline(&mut current_text);
                    if response.changed() {
                        *val = Some(current_text);
                    }
                }
                FieldValue::Password(ref mut val) => {
                    let mut current_password = val.clone().unwrap_or_default();
                    let response =
                        ui.add(egui::TextEdit::singleline(&mut current_password).password(true));
                    if response.changed() {
                        *val = Some(current_password);
                    }
                }
                FieldValue::Group(sub_fields) => {
                    ui.heading(field.display.clone().unwrap_or(field.name.clone()));
                    Self::generate_auth_ui(ui, sub_fields);
                }
            }
        }
    }

    fn draw_account_popup(&mut self, ctx: &egui::Context) {
        if !self.show_account_popup {
            return;
        }
        let is_edit = self.editing_account.is_some();
        egui::Window::new(if is_edit {
            "Edit account"
        } else {
            "New account"
        })
        .resizable(false)
        .title_bar(false)
        .collapsible(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label("Protocol");
            egui::ComboBox::from_label("Protocol")
                .selected_text(
                    self.selected_protocol
                        .map(|i| self.protocols[i].name.clone())
                        .unwrap_or_else(|| "Select protocol".into()),
                )
                .show_ui(ui, |ui| {
                    for (i, proto) in self.protocols.iter().enumerate() {
                        if ui
                            .selectable_value(&mut self.selected_protocol, Some(i), &proto.name)
                            .clicked()
                        {
                            self.temp_auth = proto.auth.clone().unwrap_or_default();
                        }
                    }
                });
            ui.separator();

            if let Some(idx) = self.selected_protocol {
                if self.protocols[idx].auth.is_some() {
                    Self::generate_auth_ui(ui, &mut self.temp_auth);
                } else {
                    ui.label("No authentication required.");
                }

                ui.separator();
                ui.heading("Private profile");

                if let Some(field) = self.temp_profile.username.as_mut() {
                    ui.horizontal(|ui| {
                        ui.label("Username:");
                        ui.text_edit_singleline(field);
                    });
                } else {
                    let mut new_username = String::new();
                    ui.horizontal(|ui| {
                        ui.label("Username:");
                        if ui.text_edit_singleline(&mut new_username).changed()
                            && !new_username.is_empty()
                        {
                            self.temp_profile.username = Some(new_username);
                        }
                    });
                }

                if let Some(field) = self.temp_profile.display_name.as_mut() {
                    ui.horizontal(|ui| {
                        ui.label("Display name:");
                        ui.text_edit_singleline(field);
                    });
                } else {
                    let mut new_display_name = String::new();
                    ui.horizontal(|ui| {
                        ui.label("Display name:");
                        if ui.text_edit_singleline(&mut new_display_name).changed()
                            && !new_display_name.is_empty()
                        {
                            self.temp_profile.display_name = Some(new_display_name);
                        }
                    });
                }

                if let Some(color) = self.temp_profile.color {
                    let [r, g, b, a] = color;
                    let mut rgba = [
                        r as f32 / 255.0,
                        g as f32 / 255.0,
                        b as f32 / 255.0,
                        a as f32 / 255.0,
                    ];
                    if ui.color_edit_button_rgba_unmultiplied(&mut rgba).changed() {
                        self.temp_profile.color = Some([
                            (rgba[0] * 255.0) as u8,
                            (rgba[1] * 255.0) as u8,
                            (rgba[2] * 255.0) as u8,
                            (rgba[3] * 255.0) as u8,
                        ]);
                    }
                } else {
                    let mut rgba = [1.0, 1.0, 1.0, 1.0];
                    if ui.color_edit_button_rgba_unmultiplied(&mut rgba).changed() {
                        self.temp_profile.color = Some([
                            (rgba[0] * 255.0) as u8,
                            (rgba[1] * 255.0) as u8,
                            (rgba[2] * 255.0) as u8,
                            (rgba[3] * 255.0) as u8,
                        ]);
                    }
                }

                if let Some(field) = self.temp_profile.picture.as_mut() {
                    ui.horizontal(|ui| {
                        ui.label("Picture URL:");
                        ui.text_edit_singleline(field);
                    });
                } else {
                    let mut new_picture = String::new();
                    ui.horizontal(|ui| {
                        ui.label("Picture URL:");
                        if ui.text_edit_singleline(&mut new_picture).changed()
                            && !new_picture.is_empty()
                        {
                            self.temp_profile.picture = Some(new_picture);
                        }
                    });
                }
            }

            ui.horizontal(|ui| {
                if ui.button(if is_edit { "Save" } else { "Add" }).clicked() {
                    if let Some(proto_idx) = self.selected_protocol {
                        let proto = &self.protocols[proto_idx];
                        let new_account = Account {
                            auth: self.temp_auth.clone(),
                            protocol_name: proto.name.clone(),
                            private_profile: Some(self.temp_profile.clone()),
                        };
                        if let Some(edit_idx) = self.editing_account {
                            self.accounts[edit_idx] = new_account;
                        } else {
                            self.accounts.push(new_account);
                        }
                        self.save_accounts();
                    }
                    self.show_account_popup = false;
                    self.editing_account = None;
                }
                if ui.button("Cancel").clicked() {
                    self.show_account_popup = false;
                    self.editing_account = None;
                }
            });
        });
    }

    fn draw_connection_popup(&mut self, ctx: &egui::Context) {
        if self.show_connection_popup {
            egui::Window::new("New connection")
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if let Some(_selected) = self.selected_account {
                            if ui.button("Create").clicked() {
                                if let Some(account_idx) = self.selected_account {
                                    let account = &self.accounts[account_idx];
                                    let protocol_name = &account.protocol_name;
                                    let conn = match protocol_name.as_str() {
                                        "sockchat" => ConnectionWrapper::Sockchat(
                                            oshatori::connection::SockchatConnection::new(),
                                        ),
                                        "Mock" => ConnectionWrapper::Mock(
                                            oshatori::connection::MockConnection::new(),
                                        ),
                                        _ => unimplemented!("Unsupported protocol"),
                                    };

                                    let conn_arc = Arc::new(Mutex::new(Box::new(conn)));
                                    self.connections.push(conn_arc.clone());
                                    let auth_cloned = account.auth.clone();

                                    let chat_log = self.chat_log.clone();
                                    let channels = self.channels.clone();
                                    let current_channel = self.current_channel.clone();
                                    let online_users = self.online_users.clone();
                                    let runtime = self.runtime.clone();

                                    runtime.spawn(async move {
                                        {
                                            let mut conn_lock = conn_arc.lock().await;
                                            if let Err(e) = conn_lock.connect(auth_cloned).await {
                                                eprintln!("Connection error: {}", e);
                                                return;
                                            }
                                        }

                                        let mut rx = {
                                            let conn_lock = conn_arc.lock().await;
                                            conn_lock.subscribe()
                                        };
                                        loop {
                                            match rx.recv().await {
                                                Ok(ConnectionEvent::Chat {
                                                    event:
                                                        ChatEvent::New {
                                                            channel_id,
                                                            message,
                                                        },
                                                }) => {
                                                    let preview = message.content;
                                                    let ts = if message.timestamp.timestamp() < 1000
                                                    {
                                                        Utc::now()
                                                    } else {
                                                        message.timestamp
                                                    };
                                                    let prof_opt = {
                                                        let users = online_users.lock().await;
                                                        message.sender_id.as_ref().and_then(|sid| {
                                                            users
                                                                .iter()
                                                                .find(|u| {
                                                                    u.id.as_ref() == Some(sid)
                                                                })
                                                                .cloned()
                                                        })
                                                    };
                                                    let chat_event = ChatEventWrapper {
                                                        channel_id: channel_id.clone(),
                                                        timestamp: ts,
                                                        sender: message
                                                            .sender_id
                                                            .clone()
                                                            .unwrap_or_else(|| "[System]".into()),
                                                        message_preview: preview,
                                                        profile: prof_opt,
                                                    };
                                                    chat_log.lock().await.push(chat_event);
                                                }

                                                Ok(ConnectionEvent::Channel { event }) => {
                                                    match event {
                                                        ChannelEvent::New { channel } => {
                                                            let mut chans = channels.lock().await;
                                                            chans.push(channel.id.clone());
                                                            let mut curr =
                                                                current_channel.lock().await;
                                                            if curr.is_none() {
                                                                *curr = Some(channel.id.clone());
                                                            }
                                                        }
                                                        ChannelEvent::ClearList => {
                                                            channels.lock().await.clear();
                                                            *current_channel.lock().await = None;
                                                        }
                                                        ChannelEvent::Remove { channel_id } => {
                                                            let mut chans = channels.lock().await;
                                                            chans.retain(|c| {
                                                                c.clone() != channel_id
                                                            });
                                                        }
                                                        ChannelEvent::Update {
                                                            channel_id,
                                                            new_channel,
                                                        } => {
                                                            let mut chans = channels.lock().await;

                                                            chans.retain(|c| {
                                                                c.clone() != channel_id
                                                            });

                                                            if !chans.contains(&new_channel.id) {
                                                                chans.push(new_channel.id.clone());
                                                            }

                                                            let mut curr =
                                                                current_channel.lock().await;
                                                            match curr.clone() {
                                                                Some(id) => {
                                                                    if id == channel_id {
                                                                        *curr = Some(
                                                                            new_channel.id.clone(),
                                                                        );
                                                                    }
                                                                }
                                                                None => {}
                                                            }
                                                        }

                                                        _ => {
                                                            eprintln!(
                                                                "Channel event received: {:?}",
                                                                event
                                                            );
                                                        }
                                                    }
                                                }

                                                Ok(ConnectionEvent::User { event }) => {
                                                    match event {
                                                        UserEvent::New {
                                                            channel_id: _,
                                                            user,
                                                        } => {
                                                            let mut users =
                                                                online_users.lock().await;
                                                            users.push(user);
                                                        }
                                                        UserEvent::ClearList { channel_id: _ } => {
                                                            online_users.lock().await.clear();
                                                        }
                                                        UserEvent::Remove { user_id } => {
                                                            let mut users =
                                                                online_users.lock().await;
                                                            users.retain(|u| {
                                                                if let Some(id) = u.id.clone() {
                                                                    id != user_id
                                                                } else {
                                                                    true
                                                                }
                                                            });
                                                        }
                                                        UserEvent::Update { user_id, new_user } => {
                                                            let mut users =
                                                                online_users.lock().await;
                                                            for user in &mut *users {
                                                                if let Some(id) = &user.id {
                                                                    if id == &user_id {
                                                                        *user = new_user.clone();
                                                                        break;
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                Ok(ConnectionEvent::Status { event }) => {
                                                    eprintln!("Status event received: {:?}", event);
                                                }
                                                Ok(other) => {
                                                    eprintln!("Other event received: {:?}", other);
                                                }
                                                Err(e) => {
                                                    eprintln!(
                                                        "Error receiving connection event: {}",
                                                        e
                                                    );
                                                    break;
                                                }
                                            }
                                        }
                                    });
                                }
                                self.show_connection_popup = false;
                            }
                        } else {
                            ui.label("Select an account first.");
                        }
                        if ui.button("Cancel").clicked() {
                            self.show_connection_popup = false;
                        }
                    });
                });
        }
    }

    pub fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.draw_top_panel(ctx);
        self.draw_account_panel(ctx);
        self.draw_channel_panel(ctx);
        self.draw_online_users_panel(ctx);
        self.draw_input_panel(ctx);
        self.draw_chat_panel(ctx);
        self.draw_account_popup(ctx);
        self.draw_connection_popup(ctx);
        ctx.request_repaint();
    }
}

impl eframe::App for ChatClient {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        Self::update(self, ctx, frame);
    }
}
