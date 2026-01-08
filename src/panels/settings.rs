use crate::app::ChatClient;
use eframe::egui;
use std::time::Duration;

pub fn draw_settings(client: &mut ChatClient, ctx: &egui::Context) {
    if !client.panels.settings {
        return;
    }

    egui::SidePanel::right("settings")
        .resizable(true)
        .default_width(220.0)
        .show(ctx, |ui| {
            ui.heading("Settings");

            ui.separator();
            ui.label("Update interval (ms):");
            let mut interval_ms = client.update_interval.as_millis() as u64;
            if ui
                .add(egui::Slider::new(&mut interval_ms, 100..=2000))
                .changed()
            {
                client.update_interval = Duration::from_millis(interval_ms);
            }

            ui.separator();
            ui.checkbox(&mut client.chat_settings.autoscroll, "Auto-scroll chat");

            ui.separator();
            ui.label("Auto-embed media:");
            ui.checkbox(&mut client.chat_settings.auto_embed_images, "images");
            ui.checkbox(&mut client.chat_settings.auto_embed_videos, "videos");
            ui.checkbox(&mut client.chat_settings.auto_embed_audio, "audio");

            ui.separator();
            ui.label("Auto-embed assets:");
            ui.checkbox(&mut client.chat_settings.auto_embed_emotes, "emotes");
            ui.checkbox(&mut client.chat_settings.auto_embed_stickers, "stickers");
        });
}
