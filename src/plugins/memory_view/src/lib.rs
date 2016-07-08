#[macro_use]
extern crate prodbg_api;

use prodbg_api::{View, Ui, Service, Reader, Writer, PluginHandler, CViewCallbacks, PDVec2, InputTextFlags, Key, ImGuiStyleVar};
use std::str;

struct MemoryCellEditor {
    address: Option<usize>,
    buf: Vec<u8>,
    should_take_focus: bool, // Needed since we cannot change focus in current frame
}

impl MemoryCellEditor {
    pub fn new() -> MemoryCellEditor {
        MemoryCellEditor {
            address: None,
            buf: vec!(0; 32),
            should_take_focus: false,
        }
    }

    pub fn change_address(&mut self, address: usize, data: &str) {
        self.address = Some(address);
        (&mut self.buf[0..data.len()]).copy_from_slice(data.as_bytes());
        self.should_take_focus = true;
    }

    pub fn set_inactive(&mut self) {
        self.address = None;
    }
}

struct MemoryView {
    data: Vec<u8>,
    start_address: usize,
    bytes_per_line: usize,
    chars_per_address: usize,
    memory_editor: MemoryCellEditor,
}

impl MemoryView {
    fn render_line(editor: &mut MemoryCellEditor, ui: &mut Ui, address: usize, data: &mut [u8]) {
        ui.text(&format!("{:#010x}", address));
        ui.same_line(0, -1);
        ui.push_style_var_vec(ImGuiStyleVar::FramePadding, PDVec2{x: 0.0, y: 0.0});
//        ui.push_style_var_vec(ImGuiStyleVar::ItemSpacing, PDVec2{x: 0.0, y: 0.0});
        let mut cur_address = address;
        let mut new_cell_value = None;
        for byte in data.iter() {
            ui.same_line(0, -1);
            if editor.address == Some(cur_address) {
                let width = ui.calc_text_size("ff", 0).0;
                if editor.should_take_focus {
                    // TODO: move cursor to start of field
                    ui.set_keyboard_focus_here(0);
                    editor.should_take_focus = false;
                }
                ui.push_item_width(width);
                let flags = InputTextFlags::CharsHexadecimal as i32|InputTextFlags::EnterReturnsTrue as i32|InputTextFlags::NoHorizontalScroll as i32|InputTextFlags::AlwaysInsertMode as i32|InputTextFlags::AlwaysInsertMode as i32;//|InputTextFlags::CallbackAlways as i32;
                if ui.input_text("##data", &mut editor.buf, flags) {
                    let text = String::from_utf8(editor.buf.clone()).unwrap();
                    let new_value = u8::from_str_radix(&text[0..2], 16).unwrap();
                    let offset = cur_address - address;
                    new_cell_value = Some((offset, new_value));
                }
                ui.pop_item_width()
            } else {
                let text = format!("{:02x}", byte);
                ui.text(&text);
                if ui.is_item_hovered() && ui.is_mouse_clicked(0, false) {
                    //TODO: send change to ProDBG
                    editor.change_address(cur_address, &text);
                }
            }
            cur_address += 1;
        }
        if let Some((offset, new_value)) = new_cell_value {
            data[offset] = new_value;
            editor.set_inactive();
        }
        let copy:Vec<u8> = data.iter().map(|byte|
            match *byte {
                32...128 => *byte,
                _ => '.' as u8,
            }
        ).collect();
        ui.same_line(0, -1);
        ui.text(str::from_utf8(&copy).unwrap());
        ui.pop_style_var(1);
//        ui.pop_style_var(2);
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
            start_address: 0,
            bytes_per_line: 8,
            chars_per_address: 10,
            memory_editor: MemoryCellEditor::new(),
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

        for line in self.data.chunks_mut(bytes_per_line) {
            Self::render_line(&mut self.memory_editor, ui, address, line);
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
