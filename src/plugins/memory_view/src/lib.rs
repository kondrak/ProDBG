#[macro_use]
extern crate prodbg_api;

use prodbg_api::{View, Ui, Service, Reader, Writer, PluginHandler, CViewCallbacks, Vec2};
use std::str;

struct MemoryView {
    data: Vec<u8>,
    bytes_per_line: usize,
    chars_per_address: usize,
}

impl MemoryView {
    fn render_line(ui: &mut Ui, address: usize, data: &[u8]) {
        ui.text(&format!("{:#010x}", address));
        ui.same_line(0, -1);
        for byte in data {
            ui.same_line(0, -1);
            ui.text(&format!("{:02x}", byte));
        }
        let copy:Vec<u8> = data.iter().map(|byte|
            match *byte {
                32...128 => *byte,
                _ => '.' as u8,
            }
        ).collect();
        ui.same_line(0, -1);
        ui.text(str::from_utf8(&copy).unwrap());
    }

    fn render_header(&mut self, ui: &mut Ui) {
//        ui.push_item_width(128.0);
        ui.text("Start Address");
        ui.same_line(0, -1);
        let mut is_auto = self.bytes_per_line == 0;
        ui.checkbox("Auto width", &mut is_auto);
        if is_auto {
            self.bytes_per_line = 0;
        } else {
            self.bytes_per_line = 16;
        }
//        ui.input_text("Size", data->sizeText, sizeof(data->sizeText), 0, 0, 0);
//        ui.pop_item_width();
    }
}

impl View for MemoryView {
    fn new(_: &Ui, _: &Service) -> Self {
        MemoryView {
            data: vec![0; 1024],
            bytes_per_line: 8,
            chars_per_address: 10,
        }
    }

    fn update(&mut self, ui: &mut Ui, _: &mut Reader, _: &mut Writer) {
        self.render_header(ui);
        let mut address = 0;
        let bytes_per_line = match self.bytes_per_line {
            0 => {
                let glyph_size = ui.calc_text_size("F", 0).0;
                let address_size = (self.chars_per_address as f32) * glyph_size;
                let screen_width = ui.get_window_size().0;
                let screen_left = screen_width - address_size;
                let chars_per_byte = 4;
                let chars_left = (screen_left / glyph_size) as i32;
                if chars_left > chars_per_byte {
                    (chars_left / chars_per_byte) as usize
                } else {
                    1
                }
            },
            _ => self.bytes_per_line,
        };
        for line in self.data.chunks(bytes_per_line) {
            Self::render_line(ui, address, line);
            address += line.len();
        }

//        PDVec2 child_size = { 0.0f, 0.0f };
//        PDVec2 windowSize = ui.get_window_size();
//
//        ui.begin_child("child", child_size, false, 0);
//
//        //PDRect rect = ui.getCurrentClipRect();
//        //PDVec2 pos = ui.get_window_pos();
//
//        //printf("pos %f %f\n", pos.x, pos.y);
//        //printf("rect %f %f %f %f\n", rect.x, rect.y, rect.width, rect.height);
//
//        // TODO: Fix me
//        const float fontWidth = 13.0f; // ui.getFontWidth();
//
//        float drawableChars = (float)(int)(windowSize.x / (fontWidth + 23));
//
//        int drawableLineCount = (int)((size) / (int)drawableChars);
//
//        //printf("%d %d %d %d\n", drawableLineCount, (int)endAddress, (int)startAddress, (int)drawableChars);
//
//        drawData(data, uiFuncs, drawableLineCount, (int)drawableChars);
//
//        ui.end_child();

    }
}

#[no_mangle]
pub fn init_plugin(plugin_handler: &mut PluginHandler) {
    define_view_plugin!(PLUGIN, b"Memory View\0", MemoryView);
    plugin_handler.register_view(&PLUGIN);
}
