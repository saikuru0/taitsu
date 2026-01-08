use crate::app::ChatClient;
use crate::utils::color32;
use eframe::egui::{self, Image, ScrollArea};

pub fn draw_users(client: &mut ChatClient, ctx: &egui::Context) {
    if !client.panels.users {
        return;
    }

    egui::SidePanel::right("users")
        .resizable(true)
        .default_width(360.0)
        .show(ctx, |ui| {
            ui.heading("Users");
            ui.separator();

            let conn = client.active_connection();

            if let Some(conn) = conn {
                let mut all_users = conn.global_users.clone();
                if let Some(ref channel_state) = conn.current_channel {
                    for (id, user) in &channel_state.users {
                        all_users.insert(id.clone(), user.clone());
                    }
                }

                if !all_users.is_empty() {
                    let mut sorted: Vec<_> = all_users.into_iter().collect();
                    sorted.sort_by(|a, b| {
                        let name_a = a.1.username.as_deref().unwrap_or("");
                        let name_b = b.1.username.as_deref().unwrap_or("");
                        name_a.to_lowercase().cmp(&name_b.to_lowercase())
                    });

                    ScrollArea::vertical().show(ui, |ui| {
                        for (_, user) in &sorted {
                            let name = user
                                .display_name
                                .as_ref()
                                .or(user.username.as_ref())
                                .map(|s| s.as_str())
                                .unwrap_or("Unknown");

                            ui.horizontal(|ui| {
                                if let Some(pic) = &user.picture {
                                    ui.add(
                                        Image::from_uri(pic)
                                            .fit_to_exact_size(egui::Vec2::new(24.0, 24.0))
                                            .rounding(egui::Rounding::same(2.0)),
                                    );
                                }

                                if let Some(color) = user.color {
                                    ui.colored_label(color32(color), name);
                                } else {
                                    ui.label(name);
                                }
                            });
                        }
                    });
                } else {
                    ui.label("No users to display");
                }
            } else {
                ui.label("No active connection");
            }
        });
}
