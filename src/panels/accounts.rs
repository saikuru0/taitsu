use crate::app::ChatClient;
use eframe::egui::{self, RichText, ScrollArea};

pub fn draw_accounts(client: &mut ChatClient, ctx: &egui::Context) {
    if !client.panels.accounts {
        return;
    }

    egui::SidePanel::left("accounts")
        .resizable(true)
        .default_width(200.0)
        .show(ctx, |ui| {
            ui.heading("Accounts");

            let cache = client.cache.lock().unwrap();
            let accounts: Vec<_> = cache
                .accounts
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    let name = a
                        .private_profile
                        .as_ref()
                        .and_then(|p| p.username.as_ref())
                        .unwrap_or(&a.protocol_name);
                    (i, format!("{} ({})", name, a.protocol_name), a.autoconnect)
                })
                .collect();

            let acc_to_conn = client.account_to_conn.lock().unwrap().clone();
            drop(cache);

            ScrollArea::vertical().show(ui, |ui| {
                for (i, name, _autoconnect) in &accounts {
                    let is_connected = acc_to_conn.contains_key(i);
                    let selected = client.selected_account == Some(*i);

                    ui.horizontal(|ui| {
                        let status_color = if is_connected {
                            egui::Color32::GREEN
                        } else {
                            egui::Color32::GRAY
                        };
                        ui.label(RichText::new("*").color(status_color));

                        if ui.selectable_label(selected, name).clicked() {
                            client.selected_account = Some(*i);
                        }
                    });
                }
            });

            ui.separator();

            ui.horizontal(|ui| {
                if ui.button("Add").clicked() {
                    client.show_account_popup = true;
                    client.editing_account = None;
                    client.temp_profile = oshatori::Profile::default();
                    client.temp_auth = Vec::new();
                    client.selected_protocol = None;
                }

                if ui.button("Edit").clicked() && client.selected_account.is_some() {
                    client.show_account_popup = true;
                    client.editing_account = client.selected_account;
                    if let Some(idx) = client.selected_account {
                        let cache = client.cache.lock().unwrap();
                        if let Some(account) = cache.accounts.get(idx) {
                            client.temp_auth = account.auth.clone();
                            client.temp_profile =
                                account.private_profile.clone().unwrap_or_default();
                            client.selected_protocol = client
                                .protocols
                                .iter()
                                .position(|p| p.name == account.protocol_name);
                        }
                    }
                }

                if ui.button("Delete").clicked() {
                    if let Some(idx) = client.selected_account {
                        {
                            let mut cache = client.cache.lock().unwrap();
                            cache.accounts.remove(idx);
                            crate::utils::save_accounts(&cache.accounts);
                        }
                        client.selected_account = None;
                    }
                }
            });

            if let Some(idx) = client.selected_account {
                let is_connected = acc_to_conn.contains_key(&idx);

                ui.separator();

                if is_connected {
                    if ui.button("Disconnect").clicked() {
                        client.disconnect_account(idx);
                    }
                } else {
                    if ui.button("Connect").clicked() {
                        let cache = client.cache.lock().unwrap();
                        if let Some(account) = cache.accounts.get(idx).cloned() {
                            drop(cache);
                            client.connect_account(idx, account);
                        }
                    }
                }

                let mut autoconnect = accounts.get(idx).map(|(_, _, ac)| *ac).unwrap_or(false);
                if ui.checkbox(&mut autoconnect, "Auto-connect").changed() {
                    let mut cache = client.cache.lock().unwrap();
                    if let Some(acc) = cache.accounts.get_mut(idx) {
                        acc.autoconnect = autoconnect;
                        crate::utils::save_accounts(&cache.accounts);
                    }
                }
            }
        });
}
