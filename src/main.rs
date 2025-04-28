pub mod gui;

use std::sync::Arc;
use tokio::runtime::Runtime;

fn main() -> eframe::Result<()> {
    let runtime = Arc::new(Runtime::new().unwrap());

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        gui::ChatClient::name(),
        native_options,
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(gui::ChatClient::new_with_runtime(runtime.clone())))
        }),
    )
}
