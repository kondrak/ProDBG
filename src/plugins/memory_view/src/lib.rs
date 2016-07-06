#[macro_use]
extern crate prodbg_api;

use prodbg_api::*;

struct BitmapView {
    dummy: i32,
}

impl View for BitmapView {
    fn new(_: &Ui, _: &Service) -> Self {
        BitmapView { dummy: 0 }
    }

    fn update(&mut self, ui: &mut Ui, _: &mut Reader, _: &mut Writer) {
        if ui.button("qq", None) {
            println!("yah");
        }

        self.dummy += 1;
    }
}

#[no_mangle]
pub fn init_plugin(plugin_handler: &mut PluginHandler) {
    define_view_plugin!(PLUGIN, b"Memory View\0", BitmapView);
    plugin_handler.register_view(&PLUGIN);
}
