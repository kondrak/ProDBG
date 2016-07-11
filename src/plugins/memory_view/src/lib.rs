#[macro_use]
extern crate prodbg_api;

use prodbg_api::{View, Ui, Service, Reader, Writer, PluginHandler, CViewCallbacks, PDVec2, InputTextFlags, Key, ImGuiStyleVar, EventType};
use std::str;
use std::ffi::CStr;

const BLOCK_SIZE: usize = 1024;
// ProDBG does not respond to requests with low addresses.
const START_ADDRESS: usize = 0xf0000;

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

struct InputText {
    // TODO: What buffer do we really need for address?
    buf: [u8; 20],
    value: usize,
}

impl InputText {
    pub fn new() -> InputText {
        let mut res = InputText {
            buf: [0; 20],
            value: 0,
        };
        res.set_value(0);
        return res;
    }

    pub fn render(&mut self, ui: &mut Ui) -> bool {
        let flags = InputTextFlags::CharsHexadecimal as i32|InputTextFlags::EnterReturnsTrue as i32|InputTextFlags::NoHorizontalScroll as i32|InputTextFlags::AlwaysInsertMode as i32|InputTextFlags::AlwaysInsertMode as i32;//|InputTextFlags::CallbackAlways as i32;
        if ui.input_text("##address", &mut self.buf, flags) {
            // TODO: can we just use original buffer instead?
            let len = self.buf.iter().position(|&b| b == 0).unwrap();
            let slice = str::from_utf8(&self.buf[0..len]).unwrap();
            let old_value = self.value;
            self.value = usize::from_str_radix(slice, 16).unwrap();
            return self.value != old_value;
        }
        return false;
    }

    pub fn get_value(&self) -> usize {
        self.value
    }

    pub fn set_value(&mut self, value: usize) {
        self.value = value;
        let data = format!("{:08x}", value);
        (&mut self.buf[0..data.len()]).copy_from_slice(data.as_bytes());
        self.buf[data.len() + 1] = 0;
    }
}

struct MemoryView {
    data: Vec<u8>,
    start_address: InputText,
    bytes_per_line: usize,
    chars_per_address: usize,
    memory_editor: MemoryCellEditor,
    memory_request: Option<(usize, usize)>,
}

impl MemoryView {
    fn render_address(ui: &mut Ui, address: usize) {
        ui.text(&format!("{:#010x}", address));
    }

    fn render_editor(ui: &mut Ui, editor: &mut MemoryCellEditor) -> Option<u8> {
        let mut new_value = None;
        ui.push_style_var_vec(ImGuiStyleVar::FramePadding, PDVec2{x: 0.0, y: 0.0});
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
            new_value = Some(u8::from_str_radix(&text[0..2], 16).unwrap());
            editor.set_inactive();
        }
        ui.pop_item_width();
        ui.pop_style_var(1);
        return new_value;
    }

    fn render_hex_byte(ui: &mut Ui, byte: u8, editor: &mut MemoryCellEditor, address: usize) {
        let text = format!("{:02x}", byte);
        ui.text(&text);
        if ui.is_item_hovered() && ui.is_mouse_clicked(0, false) {
            editor.change_address(address, &text);
        }
    }

    fn render_ansi_string(ui: &mut Ui, data: &[u8]) {
        // TODO: align data
        let copy:Vec<u8> = data.iter().map(|byte|
            match *byte {
                32...127 => *byte,
                _ => '.' as u8,
            }
        ).collect();
        ui.same_line(0, -1);
        ui.text(str::from_utf8(&copy).unwrap());
    }

    fn render_line(editor: &mut MemoryCellEditor, ui: &mut Ui, address: usize, data: &mut [u8]) {
        //TODO: Hide editor when user clicks somewhere else
        Self::render_address(ui, address);
        ui.same_line(0, -1);
        let mut cur_address = address;
        for byte in data.iter_mut() {
            ui.same_line(0, -1);
            if editor.address == Some(cur_address) {
                if let Some(new_value) = Self::render_editor(ui, editor) {
                    *byte = new_value;
                    // TODO: send change to ProDBG
                }
            } else {
                Self::render_hex_byte(ui, *byte, editor, cur_address);
            }
            cur_address += 1;
        }
        Self::render_ansi_string(ui, data);
    }

    fn render_header(&mut self, ui: &mut Ui) {
        ui.text("0x");
        ui.same_line(0, 0);
        ui.push_style_var_vec(ImGuiStyleVar::FramePadding, PDVec2{x: 1.0, y: 0.0});
        ui.push_item_width(ui.calc_text_size("00000000", 0).0 + 2.0);
        if self.start_address.render(ui) {
            self.memory_request = Some((self.start_address.get_value(), BLOCK_SIZE));
        }
        ui.pop_item_width();
        ui.pop_style_var(1);
        ui.same_line(0, -1);
        let mut is_auto = self.bytes_per_line == 0;
        ui.checkbox("Auto width", &mut is_auto);
        if is_auto {
            self.bytes_per_line = 0;
        } else {
            self.bytes_per_line = 16;
        }
//        ui.input_text("Size", data->sizeText, sizeof(data->sizeText), 0, 0, 0);
    }

    fn process_events(&mut self, reader: &mut Reader) {
        for event_type in reader.get_events() {
            match event_type {
                et if et == EventType::SetMemory as i32 => {
                    println!("Updating memory");
                    self.update_memory(reader);
                },
                _ => {}//println!("Got unknown event type: {:?}", event_type)}
            }
        }
    }

    fn update_memory(&mut self, reader: &mut Reader) {
        match reader.find_u64("address") {
            Ok(address) => {
                self.start_address.set_value(address as usize);
                println!("Setting address {}", address);
            },
            Err(err) => {
                println!("Could not get address: {:?}", err);
                return;
            }
        }
        match reader.find_data("data") {
            Ok(data) => {
                println!("Got memory. Length is {}, buf ", data.len());
                // TODO: check length here
                (&mut self.data[0..data.len()]).copy_from_slice(data);
            },
            Err(err) => {
                println!("Could not read memory: {:?}", err);
            }
        }
    }
}

impl View for MemoryView {
    fn new(_: &Ui, _: &Service) -> Self {
        MemoryView {
            data: vec![0; BLOCK_SIZE],
            start_address: InputText::new(),
            bytes_per_line: 8,
            chars_per_address: 10,
            memory_editor: MemoryCellEditor::new(),
            memory_request: Some((START_ADDRESS, BLOCK_SIZE)),
        }
    }

    fn update(&mut self, ui: &mut Ui, reader: &mut Reader, writer: &mut Writer) {
        self.process_events(reader);
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

        if let Some((address, size)) = self.memory_request {
            writer.event_begin(EventType::GetMemory as u16);
            writer.write_u64("address_start", address as u64);
            writer.write_u64("size", size as u64);
            writer.event_end();
            self.memory_request = None;
        }
    }
}

#[no_mangle]
pub fn init_plugin(plugin_handler: &mut PluginHandler) {
    define_view_plugin!(PLUGIN, b"Memory View\0", MemoryView);
    plugin_handler.register_view(&PLUGIN);
}
