use crate::app::ChatClient;
use eframe::egui::{self, ScrollArea};
use oshatori::client::ConnectionStatus;

pub fn draw_channels(client: &mut ChatClient, ctx: &egui::Context) {
    if !client.panels.channels {
        return;
    }

    egui::SidePanel::left("channels")
        .resizable(true)
        .default_width(220.0)
        .show(ctx, |ui| {
            let cache = client.cache.lock().unwrap();
            let connections: Vec<_> = cache.connections.values().cloned().collect();
            let active_conn = cache.active_connection.clone();
            let accounts = cache.accounts.clone();
            drop(cache);

            if connections.is_empty() {
                ui.heading("Channels");
                ui.separator();
                ui.label("No active connections");
                return;
            }

            ui.horizontal_wrapped(|ui| {
                for conn in &connections {
                    let is_active = active_conn.as_ref() == Some(&conn.connection_id);
                    let account_name = accounts
                        .get(conn.account_index)
                        .and_then(|a| a.private_profile.as_ref())
                        .and_then(|p| p.username.as_ref())
                        .map(|s| s.as_str())
                        .unwrap_or("Unknown");

                    let status_symbol = match conn.status {
                        ConnectionStatus::Connected => "= w=",
                        ConnectionStatus::Connecting => "o ^o",
                        ConnectionStatus::Disconnected => ". _.",
                    };

                    let tab_text = format!("({}) {}", status_symbol, account_name);

                    if ui.selectable_label(is_active, tab_text).clicked() {
                        client.set_active_connection(conn.connection_id.clone());
                    }
                }
            });

            ui.separator();
            ui.heading("Channels");
            ui.separator();

            if let Some(active_id) = &active_conn {
                if let Some(conn) = connections.iter().find(|c| &c.connection_id == active_id) {
                    ScrollArea::vertical().show(ui, |ui| {
                        for channel_id in &conn.channels {
                            let display = if channel_id.is_empty() {
                                "General"
                            } else {
                                channel_id
                            };

                            let is_current = conn
                                .current_channel
                                .as_ref()
                                .map(|ch| &ch.channel.id == channel_id)
                                .unwrap_or(false);

                            if ui.selectable_label(is_current, display).clicked() {
                                client.sync_selection(channel_id.clone());
                            }
                        }
                    });
                }
            }
        });
}
