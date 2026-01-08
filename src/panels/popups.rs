use crate::app::ChatClient;
use crate::utils::save_accounts;
use eframe::egui::{self, TextEdit, Ui};
use oshatori::{Account, AuthField, FieldValue};

pub fn draw_popups(client: &mut ChatClient, ctx: &egui::Context) {
    draw_account_popup(client, ctx);
}

fn draw_account_popup(client: &mut ChatClient, ctx: &egui::Context) {
    if !client.show_account_popup {
        return;
    }

    egui::Window::new("Account configuration")
        .collapsible(false)
        .resizable(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Protocol:");
                let selected = client
                    .selected_protocol
                    .map(|i| client.protocols[i].name.clone())
                    .unwrap_or_else(|| "Select protocol".to_string());
                egui::ComboBox::from_id_salt("protocol_combo")
                    .selected_text(selected)
                    .show_ui(ui, |ui| {
                        for (i, protocol) in client.protocols.iter().enumerate() {
                            if ui
                                .selectable_value(
                                    &mut client.selected_protocol,
                                    Some(i),
                                    &protocol.name,
                                )
                                .clicked()
                            {
                                client.temp_auth = protocol.auth.clone().unwrap_or_default();
                            }
                        }
                    });
            });

            ui.separator();
            ui.label("Client-side profile:");

            ui.horizontal(|ui| {
                ui.label("Username:");
                let mut username = client.temp_profile.username.clone().unwrap_or_default();
                ui.text_edit_singleline(&mut username);
                client.temp_profile.username = if username.is_empty() {
                    None
                } else {
                    Some(username)
                };
            });

            ui.horizontal(|ui| {
                ui.label("Display name:");
                let mut display = client.temp_profile.display_name.clone().unwrap_or_default();
                ui.text_edit_singleline(&mut display);
                client.temp_profile.display_name = if display.is_empty() {
                    None
                } else {
                    Some(display)
                };
            });

            ui.separator();
            ui.label("Authentication:");

            if let Some(idx) = client.selected_protocol {
                if client.temp_auth.is_empty() {
                    if let Some(auth) = &client.protocols[idx].auth {
                        client.temp_auth = auth.clone();
                    }
                }
            }

            if !client.temp_auth.is_empty() {
                let mut temp = client.temp_auth.clone();
                auth_ui(ui, &mut temp);
                client.temp_auth = temp;
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    if let Some(idx) = client.selected_protocol {
                        let account = Account {
                            auth: client.temp_auth.clone(),
                            protocol_name: client.protocols[idx].name.clone(),
                            private_profile: Some(client.temp_profile.clone()),
                            autoconnect: false,
                        };

                        {
                            let mut cache = client.cache.lock().unwrap();
                            if let Some(edit_idx) = client.editing_account {
                                cache.accounts[edit_idx] = account;
                            } else {
                                cache.accounts.push(account);
                            }
                            save_accounts(&cache.accounts);
                        }

                        client.show_account_popup = false;
                    }
                }

                if ui.button("Cancel").clicked() {
                    client.show_account_popup = false;
                }
            });
        });
}

fn auth_ui(ui: &mut Ui, fields: &mut Vec<AuthField>) {
    for field in fields.iter_mut() {
        ui.horizontal(|ui| {
            let label = field.display.as_ref().unwrap_or(&field.name);
            ui.label(format!("{}:", label));

            match &mut field.value {
                FieldValue::Text(text) => {
                    let mut val = text.clone().unwrap_or_default();
                    ui.text_edit_singleline(&mut val);
                    *text = Some(val);
                }
                FieldValue::Password(password) => {
                    let mut val = password.clone().unwrap_or_default();
                    ui.add(TextEdit::singleline(&mut val).password(true));
                    *password = Some(val);
                }
                FieldValue::Group(nested) => {
                    ui.vertical(|ui| {
                        auth_ui(ui, nested);
                    });
                }
            }
        });
    }
}
