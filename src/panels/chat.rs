use crate::app::ChatClient;
use crate::state::ChatSettings;
use crate::utils::color32;
use chrono::Utc;
use eframe::egui::{self, Color32, Image, RichText, ScrollArea, TextEdit, Ui};
use oshatori::{
    client::ConnectionStatus,
    connection::{ChatEvent, ConnectionEvent},
    Asset, Message, MessageFragment, MessageStatus, MessageType,
};

pub fn draw_chat(client: &mut ChatClient, ctx: &egui::Context) {
    if client.panels.chat {
        egui::CentralPanel::default().show(ctx, |ui| {
            let conn = client.active_connection();

            if let Some(ref conn) = conn {
                if conn.status == ConnectionStatus::Connecting {
                    ui.vertical_centered(|ui| {
                        ui.add_space(50.0);
                        ui.heading(
                            RichText::new("Connecting...")
                                .size(32.0)
                                .color(Color32::YELLOW),
                        );
                        ui.add_space(10.0);
                        ui.label(RichText::new("Please wait").color(Color32::GRAY));
                        ui.add_space(50.0);
                    });
                    return;
                }
            }

            let available_height = {
                if client.panels.input {
                    ui.available_height() - 80.0
                } else {
                    ui.available_height()
                }
            };

            let current_message_count = conn
                .as_ref()
                .and_then(|c| c.current_channel.as_ref())
                .map(|ch| ch.messages.len())
                .unwrap_or(0)
                + conn.as_ref().map(|c| c.pending_messages.len()).unwrap_or(0);

            let should_scroll = client.chat_settings.autoscroll
                && current_message_count > client.chat_settings.last_message_count;

            let scroll_output = ScrollArea::vertical()
                .max_height(available_height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());

                    if let Some(conn) = &conn {
                        if let Some(channel_state) = &conn.current_channel {
                            let mut all_users = conn.global_users.clone();
                            for (id, user) in &channel_state.users {
                                all_users.insert(id.clone(), user.clone());
                            }

                            let mut last_sender_id: Option<String> = None;
                            let mut last_message_type: Option<MessageType> = None;

                            for msg in &channel_state.messages {
                                let mut is_consecutive =
                                    last_sender_id.as_ref() == msg.sender_id.as_ref();

                                if let Some(last_type) = last_message_type {
                                    if last_type != msg.message_type
                                        || msg.message_type == MessageType::Server
                                        || msg.message_type == MessageType::Meta
                                    {
                                        is_consecutive = false;
                                    }
                                }
                                if msg.message_type == MessageType::Server
                                    || msg.message_type == MessageType::Meta
                                {
                                    is_consecutive = false;
                                }

                                draw_message(
                                    ui,
                                    msg,
                                    &all_users,
                                    &conn.assets,
                                    is_consecutive,
                                    &client.chat_settings,
                                );
                                last_sender_id = msg.sender_id.clone();
                                last_message_type = Some(msg.message_type.clone());
                            }

                            for msg in &conn.pending_messages {
                                let mut is_consecutive =
                                    last_sender_id.as_ref() == msg.sender_id.as_ref();
                                if let Some(last_type) = last_message_type {
                                    if last_type != msg.message_type
                                        || msg.message_type == MessageType::Server
                                        || msg.message_type == MessageType::Meta
                                    {
                                        is_consecutive = false;
                                    }
                                }

                                draw_message(
                                    ui,
                                    msg,
                                    &all_users,
                                    &conn.assets,
                                    is_consecutive,
                                    &client.chat_settings,
                                );
                                last_sender_id = msg.sender_id.clone();
                                last_message_type = Some(msg.message_type.clone());
                            }

                            if should_scroll {
                                ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                            }
                        } else {
                            ui.label("Select a channel to start chatting");
                        }
                    } else {
                        ui.vertical_centered(|ui| {
                            ui.add_space(50.0);
                            ui.heading(RichText::new("No active connection").color(Color32::GRAY));
                            ui.add_space(10.0);
                            ui.label("Connect to an account to start chatting");
                        });
                    }
                });

            client.chat_settings.last_message_count = current_message_count;
            let _ = scroll_output;

            ui.separator();

            if client.panels.input {
                if let Some(conn) = &conn {
                    if let Some(user) = &conn.current_user {
                        let name = user
                            .display_name
                            .as_ref()
                            .or(user.username.as_ref())
                            .map(|s| s.as_str())
                            .unwrap_or("Unknown");
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("Sending as: {}", name))
                                    .small()
                                    .color(Color32::GRAY),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button("UNEMBED ALL").clicked() {
                                        client.chat_settings.embed_generation += 1;
                                        client.chat_settings.unembed_override = true;
                                    }
                                    ui.checkbox(&mut client.chat_settings.autoscroll, "Autoscroll");
                                },
                            );
                        });
                    }
                }

                ui.horizontal(|ui| {
                    if ui.button("+").on_hover_text("Insert asset").clicked() {
                        client.show_asset_picker = !client.show_asset_picker;
                    }

                    let response = ui.add(
                        TextEdit::singleline(&mut client.new_message)
                            .desired_width(ui.available_width() - 80.0)
                            .hint_text("Type a message..."),
                    );

                    let send = ui.button("Send").clicked()
                        || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));

                    if send && !client.new_message.trim().is_empty() {
                        response.request_focus();
                        if let Some(conn) = client.active_connection() {
                            let sender_id = conn.current_user.as_ref().and_then(|u| u.id.clone());

                            let message = Message {
                                id: None,
                                sender_id: sender_id.clone(),
                                content: vec![MessageFragment::Text(client.new_message.clone())],
                                timestamp: Utc::now(),
                                message_type: MessageType::CurrentUser,
                                status: MessageStatus::Sent,
                            };

                            client.add_pending_message(message.clone());

                            let event = ConnectionEvent::Chat {
                                event: ChatEvent::New {
                                    channel_id: conn
                                        .current_channel
                                        .as_ref()
                                        .map(|ch| ch.channel.id.clone()),
                                    message,
                                },
                            };

                            client.send_event(event);
                            client.new_message.clear();
                        }
                    }
                });
            }

            if client.show_asset_picker {
                if let Some(conn) = &conn {
                    draw_asset_picker(client, ctx, &conn.assets);
                }
            }
        });
    }
}

fn draw_asset_picker(
    client: &mut ChatClient,
    ctx: &egui::Context,
    assets: &std::collections::HashMap<String, Asset>,
) {
    egui::Window::new("Insert asset")
        .collapsible(true)
        .resizable(true)
        .movable(true)
        .default_size([300.0, 350.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Available assets");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("x").clicked() {
                        client.show_asset_picker = false;
                    }
                });
            });
            ui.separator();

            if assets.is_empty() {
                ui.label("No assets available for this connection");
                return;
            }

            let mut emotes: Vec<_> = assets
                .iter()
                .filter_map(|(id, a)| {
                    if let Asset::Emote { pattern, src, .. } = a {
                        Some((id.clone(), pattern.clone(), src.clone()))
                    } else {
                        None
                    }
                })
                .collect();
            emotes.sort_by(|a, b| a.1.cmp(&b.1));

            let mut stickers: Vec<_> = assets
                .iter()
                .filter_map(|(id, a)| {
                    if let Asset::Sticker { pattern, src, .. } = a {
                        Some((id.clone(), pattern.clone(), src.clone()))
                    } else {
                        None
                    }
                })
                .collect();
            stickers.sort_by(|a, b| a.1.cmp(&b.1));

            let mut audio: Vec<_> = assets
                .iter()
                .filter_map(|(id, a)| {
                    if let Asset::Audio { pattern, .. } = a {
                        Some((id.clone(), pattern.clone()))
                    } else {
                        None
                    }
                })
                .collect();
            audio.sort_by(|a, b| a.1.cmp(&b.1));

            ScrollArea::vertical().show(ui, |ui| {
                if !emotes.is_empty() {
                    ui.label(RichText::new("Emotes").strong());
                    ui.horizontal_wrapped(|ui| {
                        for (id, pattern, src) in &emotes {
                            let response = if !src.is_empty() {
                                ui.add(
                                    Image::from_uri(src)
                                        .fit_to_exact_size(egui::Vec2::new(32.0, 32.0))
                                        .sense(egui::Sense::click()),
                                )
                                .on_hover_text(pattern)
                            } else {
                                ui.button(pattern).on_hover_text(id)
                            };
                            if response.clicked() {
                                client.new_message.push_str(&format!("{}", pattern));
                                client.show_asset_picker = false;
                            }
                        }
                    });
                    ui.add_space(10.0);
                }

                if !stickers.is_empty() {
                    ui.label(RichText::new("Stickers").strong());
                    ui.horizontal_wrapped(|ui| {
                        for (id, pattern, src) in &stickers {
                            let response = if !src.is_empty() {
                                ui.add(
                                    Image::from_uri(src)
                                        .max_size(egui::Vec2::new(64.0, 64.0))
                                        .sense(egui::Sense::click()),
                                )
                                .on_hover_text(pattern)
                            } else {
                                ui.button(pattern).on_hover_text(id)
                            };
                            if response.clicked() {
                                client.new_message.push_str(&format!("{}", pattern));
                                client.show_asset_picker = false;
                            }
                        }
                    });
                    ui.add_space(10.0);
                }

                if !audio.is_empty() {
                    ui.label(RichText::new("Audio").strong());
                    for (id, pattern) in &audio {
                        if ui
                            .button(format!("[audio] {}", pattern))
                            .on_hover_text(id)
                            .clicked()
                        {
                            client.new_message.push_str(&format!("{}", pattern));
                            client.show_asset_picker = false;
                        }
                    }
                }
            });
        });
}

fn draw_message(
    ui: &mut Ui,
    msg: &Message,
    users: &std::collections::HashMap<String, oshatori::Profile>,
    assets: &std::collections::HashMap<String, Asset>,
    is_consecutive: bool,
    settings: &ChatSettings,
) {
    let sender = msg.sender_id.as_ref().and_then(|id| users.get(id));

    if matches!(msg.message_type, MessageType::Server | MessageType::Meta) {
        ui.horizontal(|ui| {
            ui.add_space(42.0);
            ui.vertical(|ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(msg.timestamp.format("%H:%M:%S").to_string())
                            .color(Color32::from_gray(100))
                            .small(),
                    );

                    ui.scope(|ui| {
                        ui.style_mut().visuals.override_text_color = Some(Color32::from_gray(150));
                        for (i, fragment) in msg.content.iter().enumerate() {
                            draw_fragment(ui, fragment, assets, settings, msg.id.as_ref(), i);
                        }
                    });
                });
            });
        });
        return;
    }

    if is_consecutive {
        ui.horizontal(|ui| {
            ui.add_space(40.0);
            ui.vertical(|ui| {
                ui.horizontal_wrapped(|ui| {
                    for (i, fragment) in msg.content.iter().enumerate() {
                        draw_fragment(ui, fragment, assets, settings, msg.id.as_ref(), i);
                    }
                });
            });
        });
    } else {
        ui.separator();
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if let Some(profile) = sender {
                if let Some(pic) = &profile.picture {
                    ui.add(
                        Image::from_uri(pic)
                            .fit_to_exact_size(egui::Vec2::new(32.0, 32.0))
                            .rounding(egui::Rounding::same(2.0)),
                    );
                } else {
                    ui.add_space(32.0);
                }
            } else {
                ui.add_space(32.0);
            }

            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    let default = "Unknown".to_string();
                    let sender_id = msg.sender_id.as_ref().unwrap_or(&default);
                    let name = sender
                        .and_then(|p| p.display_name.as_ref().or(p.username.as_ref()))
                        .unwrap_or(sender_id);

                    let color = sender
                        .and_then(|p| p.color.map(color32))
                        .unwrap_or_else(|| match msg.message_type {
                            MessageType::CurrentUser => Color32::GREEN,
                            MessageType::Normal => Color32::WHITE,
                            MessageType::Server => Color32::YELLOW,
                            MessageType::Meta => Color32::GRAY,
                        });

                    ui.label(RichText::new(name).color(color).strong());
                    ui.label(
                        RichText::new(msg.timestamp.format("@ %H:%M:%S").to_string())
                            .color(Color32::GRAY)
                            .small(),
                    );
                });

                ui.horizontal_wrapped(|ui| {
                    for (i, fragment) in msg.content.iter().enumerate() {
                        draw_fragment(ui, fragment, assets, settings, msg.id.as_ref(), i);
                    }
                });
            });
        });
    }
}

fn draw_fragment(
    ui: &mut Ui,
    fragment: &MessageFragment,
    assets: &std::collections::HashMap<String, Asset>,
    settings: &ChatSettings,
    msg_id: Option<&String>,
    index: usize,
) {
    match fragment {
        MessageFragment::Text(text) => {
            ui.label(text);
        }
        MessageFragment::Image { url, .. } => {
            let id = ui.make_persistent_id(format!(
                "img_{}_{}_{}",
                msg_id.unwrap_or(&"none".to_string()),
                index,
                settings.embed_generation
            ));
            let default = if settings.unembed_override {
                false
            } else {
                settings.auto_embed_images
            };
            let mut show = ui.data(|d| d.get_temp(id).unwrap_or(default));

            ui.data_mut(|d| d.insert_temp(id, show));

            ui.horizontal(|ui| {
                if ui.small_button(if show { "-" } else { "+" }).clicked() {
                    show = !show;
                    ui.data_mut(|d| d.insert_temp(id, show));
                }

                if !show {
                    ui.hyperlink_to("[image]", url);
                }
            });

            if show {
                ui.add(
                    Image::from_uri(url)
                        .max_width(ui.available_width().min(600.0).max(200.0))
                        .max_height(600.0)
                        .fit_to_original_size(1.0),
                );
            }
        }
        MessageFragment::Video { url, .. } => {
            let id = ui.make_persistent_id(format!(
                "vid_{}_{}_{}",
                msg_id.unwrap_or(&"none".to_string()),
                index,
                settings.embed_generation
            ));
            let default = if settings.unembed_override {
                false
            } else {
                settings.auto_embed_videos
            };
            let mut show = ui.data(|d| d.get_temp(id).unwrap_or(default));
            ui.data_mut(|d| d.insert_temp(id, show));

            ui.horizontal(|ui| {
                if ui.small_button(if show { "-" } else { "+" }).clicked() {
                    show = !show;
                    ui.data_mut(|d| d.insert_temp(id, show));
                }
                if !show {
                    ui.hyperlink_to("[video]", url);
                }
            });

            if show {
                ui.label(RichText::new("(Video embed placeholder)").italics());
            }
        }
        MessageFragment::Audio { url, .. } => {
            let id = ui.make_persistent_id(format!(
                "aud_{}_{}_{}",
                msg_id.unwrap_or(&"none".to_string()),
                index,
                settings.embed_generation
            ));
            let default = if settings.unembed_override {
                false
            } else {
                settings.auto_embed_audio
            };
            let mut show = ui.data(|d| d.get_temp(id).unwrap_or(default));
            ui.data_mut(|d| d.insert_temp(id, show));

            ui.horizontal(|ui| {
                if ui.small_button(if show { "-" } else { "+" }).clicked() {
                    show = !show;
                    ui.data_mut(|d| d.insert_temp(id, show));
                }
                if !show {
                    ui.hyperlink_to("[audio]", url);
                }
            });

            if show {
                ui.label(RichText::new("(Audio embed placeholder)").italics());
            }
        }
        MessageFragment::Url(url) => {
            ui.hyperlink(url);
        }
        MessageFragment::AssetId(id) => {
            if let Some(asset) = assets.get(id) {
                draw_asset(ui, asset, id, settings);
            } else {
                ui.label(RichText::new(format!("[asset] {}", id)).color(Color32::GRAY));
            }
        }
    }
}

fn draw_asset(ui: &mut Ui, asset: &Asset, _id: &str, settings: &ChatSettings) {
    match asset {
        Asset::Emote { src, pattern, .. } => {
            if settings.auto_embed_emotes {
                if src.is_empty() {
                    ui.label(RichText::new(pattern).color(Color32::YELLOW));
                } else {
                    ui.add(
                        Image::from_uri(src)
                            .fit_to_exact_size(egui::Vec2::new(24.0, 24.0))
                            .rounding(egui::Rounding::same(2.0)),
                    );
                }
            } else {
                ui.label(RichText::new(pattern).color(Color32::YELLOW));
            }
        }
        Asset::Sticker { src, pattern, .. } => {
            if settings.auto_embed_stickers {
                if src.is_empty() {
                    ui.label(RichText::new(pattern).color(Color32::YELLOW));
                } else {
                    ui.add(
                        Image::from_uri(src)
                            .max_size(egui::Vec2::new(120.0, 120.0))
                            .fit_to_original_size(1.0),
                    );
                }
            } else {
                ui.label(RichText::new(format!("[sticker] {}", pattern)).color(Color32::YELLOW));
            }
        }
        Asset::Audio { src, pattern, .. } => {
            if settings.auto_embed_audio && !src.is_empty() {
                ui.hyperlink_to(format!("[audio] {}", pattern), src);
            } else {
                ui.label(RichText::new(format!("[audio] {}", pattern)).color(Color32::YELLOW));
            }
        }
        Asset::Command { args, .. } => {
            let arg_str: Vec<String> = args
                .iter()
                .map(|f| match f {
                    MessageFragment::Text(t) => t.clone(),
                    MessageFragment::Url(u) => u.clone(),
                    _ => "[asset]".to_string(),
                })
                .collect();
            ui.label(
                RichText::new(format!("/{}", arg_str.join(" ")))
                    .color(Color32::from_rgb(100, 150, 255))
                    .monospace(),
            );
        }
    }
}
