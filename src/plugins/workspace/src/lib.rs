#[macro_use]
extern crate prodbg_api;

use prodbg_api::{Ui, Service, PluginHandler, View, Reader, Writer, CViewCallbacks};

/*
struct DirEntry {
    name: String,
    dirs: Vec<DirEntry>,
    files: Vec<String>,
}
*/

struct WorkspaceView {
    _dummy: i32,
}

impl View for WorkspaceView {
    fn new(_: &Ui, _service: &Service) -> Self {
        WorkspaceView {
            _dummy: 0,
        }
    }

    fn update(&mut self, ui: &mut Ui, _reader: &mut Reader, _writer: &mut Writer) {
        ui.text("foobar");
    }
}

#[no_mangle]
pub fn init_plugin(plugin_handler: &mut PluginHandler) {
    define_view_plugin!(PLUGIN, b"Workspace\0", WorkspaceView);
    plugin_handler.register_view(&PLUGIN);
}

