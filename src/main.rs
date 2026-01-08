mod app;
mod panels;
mod state;
mod utils;

use std::sync::Arc;
use tokio::runtime::Runtime;

fn main() -> eframe::Result<()> {
    let runtime = Arc::new(Runtime::new().unwrap());

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        app::ChatClient::name(),
        native_options,
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(app::ChatClient::new(runtime.clone())))
        }),
    )
}
