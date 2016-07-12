#[macro_use]
extern crate prodbg_api;

use prodbg_api::{View, Ui, Service, Reader, Writer, PluginHandler, CViewCallbacks, PDVec2, InputTextFlags, Key, ImGuiStyleVar, EventType, InputTextCallbackData};
use std::str;

const BLOCK_SIZE: usize = 1024;
// ProDBG does not respond to requests with low addresses.
const START_ADDRESS: usize = 0xf0000;

struct HexMemoryEditor {
    address: Option<usize>, // Address for first digit
    digit_count: usize, // Amount of digits edited
    cursor: usize, // Position of digit edited (0 = lowest address)
    buf: [u8; 2], // Buffer for ImGui. Only one digit allowed
    text: String, // Text representation of buffer
    should_take_focus: bool, // Needed since we cannot change focus in current frame
    should_set_pos_to_start: bool, // Needed since we cannot change cursor position in next frame
}

impl HexMemoryEditor {
    pub fn new() -> HexMemoryEditor {
        HexMemoryEditor {
            address: None,
            digit_count: 2,
            cursor: 0,
            buf: [0; 2],
            text: String::new(),
            should_take_focus: false,
            should_set_pos_to_start: false,
        }
    }

    pub fn change_address(&mut self, address: usize, cursor: usize, data: &str) {
        self.address = Some(address);
        self.cursor = cursor;
        self.text = data.to_owned();
        self.buf[0] = data.as_bytes()[cursor];
        self.buf[1] = 0;
        self.should_take_focus = true;
        self.should_set_pos_to_start = true;
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
        if ui.input_text("##address", &mut self.buf, flags, None) {
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

#[derive(Debug, Clone, Copy)]
pub enum DataSize {
    OneByte,
    TwoBytes,
    FourBytes,
    EightBytes,
}

impl DataSize {
    /// String representation of this `DataSize`
    pub fn as_str(&self) -> &'static str {
        match *self {
            DataSize::OneByte => "1 byte",
            DataSize::TwoBytes => "2 bytes",
            DataSize::FourBytes => "4 bytes",
            DataSize::EightBytes => "8 bytes",
        }
    }

    /// Number of bytes represented by this `DataSize`
    pub fn byte_count(&self) -> i32 {
        match *self {
            DataSize::OneByte => 1,
            DataSize::TwoBytes => 2,
            DataSize::FourBytes => 4,
            DataSize::EightBytes => 8,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum DataView {
    Hex(DataSize),
    Integer(DataSize, bool),
    Float(DataSize),
}

impl DataView {
    /// String representation of `DataView`
    pub fn as_str(&self) -> &'static str {
        match *self {
            DataView::Hex(DataSize::OneByte) => "Hex 1 byte",
            DataView::Hex(DataSize::TwoBytes) => "Hex 2 bytes",
            DataView::Hex(DataSize::FourBytes) => "Hex 4 bytes",
            DataView::Hex(DataSize::EightBytes) => "Hex 8 bytes",
            DataView::Integer(DataSize::OneByte, false) => "Int 1 byte",
            DataView::Integer(DataSize::OneByte, true) => "Int 1 byte signed",
            DataView::Integer(DataSize::TwoBytes, false) => "Int 2 bytes",
            DataView::Integer(DataSize::TwoBytes, true) => "Int 2 byte signed",
            DataView::Integer(DataSize::FourBytes, false) => "Int 4 bytes",
            DataView::Integer(DataSize::FourBytes, true) => "Int 4 bytes signed",
            DataView::Integer(DataSize::EightBytes, false) => "Int 8 bytes",
            DataView::Integer(DataSize::EightBytes, true) => "Int 8 bytes signed",
            DataView::Float(DataSize::OneByte) => "Float 1 byte",
            DataView::Float(DataSize::TwoBytes) => "Float 2 bytes",
            DataView::Float(DataSize::FourBytes) => "Float 4 bytes",
            DataView::Float(DataSize::EightBytes) => "Float 8 bytes",
        }
    }

    /// Maximum number of chars needed to show number in this `DataView`
    // TODO: change to calculation from MAX/MIN when `const fn` is in stable Rust
    pub fn maximum_chars_needed(&self) -> i32 {
        match *self {
            DataView::Hex(s) => s.byte_count() * 2,
            DataView::Integer(DataSize::OneByte, false) => 3,
            DataView::Integer(DataSize::OneByte, true) => 4,
            DataView::Integer(DataSize::TwoBytes, false) => 5,
            DataView::Integer(DataSize::TwoBytes, true) => 6,
            DataView::Integer(DataSize::FourBytes, false) => 10,
            DataView::Integer(DataSize::FourBytes, true) => 11,
            DataView::Integer(DataSize::EightBytes, false) => 20,
            DataView::Integer(DataSize::EightBytes, true) => 20,
            DataView::Float(s) => s.byte_count() * 2,
        }
    }

    // TODO: change to usize
    pub fn byte_count(&self) -> i32 {
        match *self {
            DataView::Hex(s) => s.byte_count(),
            DataView::Integer(s, _) => s.byte_count(),
            DataView::Float(s) => s.byte_count(),
        }
    }

    pub fn format(&self, buffer: &[u8]) -> String {
        macro_rules! format_buffer {
            ($data_type:ty, $len:expr, $format:expr) => {
                let mut buf_copy = [0; $len];
                buf_copy.copy_from_slice(&buffer[0..$len]);
                unsafe {
                    let num: $data_type = std::mem::transmute(buf_copy);
                    return format!($format, num);
                }
            };
        }
        match *self {
            DataView::Hex(DataSize::OneByte) => {format_buffer!(u8, 1, "{:02x}");}
            DataView::Hex(DataSize::TwoBytes) => {format_buffer!(u16, 2, "{:04x}");}
            DataView::Hex(DataSize::FourBytes) => {format_buffer!(u32, 4, "{:08x}");}
            DataView::Hex(DataSize::EightBytes) => {format_buffer!(u64, 8, "{:016x}");}
            DataView::Integer(DataSize::OneByte, false) => {format_buffer!(u8, 1, "{:3}");}
            DataView::Integer(DataSize::OneByte, true) => {format_buffer!(i8, 1, "{:4}");}
            DataView::Integer(DataSize::TwoBytes, false) => {format_buffer!(u16, 2, "{:5}");}
            DataView::Integer(DataSize::TwoBytes, true) => {format_buffer!(i16, 2, "{:6}");}
            DataView::Integer(DataSize::FourBytes, false) => {format_buffer!(u32, 4, "{:10}");}
            DataView::Integer(DataSize::FourBytes, true) => {format_buffer!(i32, 4, "{:11}");}
            DataView::Integer(DataSize::EightBytes, false) => {format_buffer!(u64, 8, "{:20}");}
            DataView::Integer(DataSize::EightBytes, true) => {format_buffer!(i64, 8, "{:20}");}
            DataView::Float(DataSize::FourBytes) => {format_buffer!(f32, 4, "{}");}
            DataView::Float(DataSize::EightBytes) => {format_buffer!(f64, 8, "{}");}
            _ => return "Error".to_owned()
        }
    }
}

struct MemoryView {
    data: Vec<u8>,
    start_address: InputText,
    bytes_per_line: usize,
    chars_per_address: usize,
    memory_editor: HexMemoryEditor,
    memory_request: Option<(usize, usize)>,
    data_view: DataView
}

impl MemoryView {
    fn render_address(ui: &mut Ui, address: usize) {
        ui.text(&format!("{:#010x}", address));
    }

    fn render_memory_editor(ui: &mut Ui, editor: &mut HexMemoryEditor, unit: &mut [u8]) -> bool {
        ui.push_style_var_vec(ImGuiStyleVar::ItemSpacing, PDVec2{x: 0.0, y: 0.0});
        // TODO: this can cause panic if text somewhy non-ASCII. Can we change this somehow?
        if editor.cursor > 0 {
            let left = &editor.text[0..editor.cursor];
            ui.text(left);
        }

        let mut new_digit = None;
        let width = ui.calc_text_size("f", 0).0;
        if editor.should_take_focus {
            ui.set_keyboard_focus_here(0);
            editor.should_take_focus = false;
        }
        let flags = InputTextFlags::CharsHexadecimal as i32|InputTextFlags::NoHorizontalScroll as i32|InputTextFlags::AlwaysInsertMode as i32|InputTextFlags::CallbackAlways as i32;
        let mut should_set_pos_to_start = editor.should_set_pos_to_start;
        let mut cursor_pos = 0;
        {
            let callback = |mut data: InputTextCallbackData| {
                if should_set_pos_to_start {
                    data.set_cursor_pos(0);
                    should_set_pos_to_start = false;
                } else {
                    cursor_pos = data.get_cursor_pos();
                }
            };
            ui.same_line(0, -1);
            ui.push_item_width(width);
            ui.push_style_var_vec(ImGuiStyleVar::FramePadding, PDVec2{x: 0.0, y: 0.0});
            ui.input_text("##data", &mut editor.buf, flags, Some(&callback));
            ui.pop_style_var(1);
            ui.pop_item_width();
        }
        if cursor_pos > 0 {
            let text = String::from_utf8(editor.buf.iter().take(1).map(|x| *x).collect()).unwrap();
            new_digit = Some(u8::from_str_radix(&text, 16).unwrap());
            editor.set_inactive();
        }
        editor.should_set_pos_to_start = should_set_pos_to_start;

        if let Some(value) = new_digit {
            let offset = (editor.digit_count - editor.cursor) / 2;
            unit[offset] = if editor.cursor % 2 == 0 {
                unit[offset] & 0b00001111 | (value << 4)
            } else {
                unit[offset] & 0b11110000 | value
            };
        }

        if editor.cursor < editor.digit_count {
            ui.same_line(0, -1);
            let right = &editor.text[editor.cursor + 1..editor.digit_count];
            ui.text(right);
        }

        ui.pop_style_var(1);
        return new_digit.is_some();
    }

    fn render_unit(ui: &mut Ui, unit: &[u8], editor: &mut HexMemoryEditor, address: usize, view: DataView) {
        let text = view.format(unit);
        ui.text(&text);
        if ui.is_item_hovered() && ui.is_mouse_clicked(0, false) {
            editor.change_address(address, 1, &text);
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

    fn render_line(editor: &mut HexMemoryEditor, ui: &mut Ui, address: usize, data: &mut [u8], view: DataView) {
        //TODO: Hide editor when user clicks somewhere else
        MemoryView::render_address(ui, address);
        ui.same_line(0, -1);
        let bytes_per_unit = view.byte_count();
        let mut cur_address = address;
        for unit in data.chunks_mut(bytes_per_unit as usize) {
            ui.same_line(0, -1);
            if editor.address == Some(cur_address) {
                if MemoryView::render_memory_editor(ui, editor, unit) {
                    // TODO: send change to ProDBG
                    editor.text = view.format(unit);
                }
            } else {
                MemoryView::render_unit(ui, unit, editor, cur_address, view);
            }
            cur_address += bytes_per_unit as usize;
        }
        MemoryView::render_ansi_string(ui, data);
    }

    fn change_data_view(&mut self, view: DataView) {
        self.data_view = view;
        self.memory_editor.set_inactive();
        self.memory_editor.digit_count = (view.byte_count() * 2) as usize;
    }

    fn render_data_view_picker(&mut self, ui: &mut Ui) {
        if ui.button(self.data_view.as_str(), None) {
            ui.open_popup("##data_view");
        }
        macro_rules! data_view_menu {
            (Integer, $name:expr => $($size:ident),+) => {
                if ui.begin_menu($name, true) {
                    $(if ui.begin_menu(DataSize::$size.as_str(), true) {
                        if ui.menu_item("Unsigned", false, true) {
                            self.change_data_view(DataView::Integer(DataSize::$size, false));
                        }
                        if ui.menu_item("Signed", false, true) {
                            self.change_data_view(DataView::Integer(DataSize::$size, true));
                        }
                        ui.end_menu();
                    })+
                    ui.end_menu();
                }
            };
            ($variant:ident, $name:expr => $($size:ident),+) => {
                if ui.begin_menu($name, true) {
                    $(if ui.menu_item(DataSize::$size.as_str(), false, true) {
                        self.change_data_view(DataView::$variant(DataSize::$size));
                    })+
                    ui.end_menu();
                }
            };
        }
        if ui.begin_popup("##data_view") {
            data_view_menu!(Hex, "Hex" => OneByte, TwoBytes, FourBytes, EightBytes);
            data_view_menu!(Integer, "Integer" => OneByte, TwoBytes, FourBytes, EightBytes);
            data_view_menu!(Float, "Float" => FourBytes, EightBytes);
            ui.end_popup();
        }
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
        self.render_data_view_picker(ui);
        ui.same_line(0, -1);
        let mut is_auto = self.bytes_per_line == 0;
        ui.checkbox("Auto width", &mut is_auto);
        if is_auto {
            self.bytes_per_line = 0;
        } else {
            self.bytes_per_line = 16;
        }
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
            memory_editor: HexMemoryEditor::new(),
            memory_request: Some((START_ADDRESS, BLOCK_SIZE)),
            data_view: DataView::Hex(DataSize::OneByte),
        }
    }

    fn update(&mut self, ui: &mut Ui, reader: &mut Reader, writer: &mut Writer) {
        self.process_events(reader);
        self.render_header(ui);
        let mut address = 0;
        let bytes_per_line = match self.bytes_per_line {
            0 => {
                let glyph_size = ui.calc_text_size("F", 0).0;
                // Size of column with address (address length + space)
                let address_size = (self.chars_per_address as f32 + 1.0) * glyph_size;
                let screen_width = ui.get_window_size().0;
                // Screen space available for int and chars view
                let screen_left = screen_width - address_size;
                // Number of chars we can draw
                let chars_left = (screen_left / glyph_size) as i32;
                // Number of chars we need to draw one unit
                let unit_size = self.data_view.byte_count();
                let chars_per_unit = self.data_view.maximum_chars_needed() + 1 + unit_size;
                if chars_left > chars_per_unit {
                    (chars_left / chars_per_unit * unit_size) as usize
                } else {
                    unit_size as usize
                }
            },
            _ => self.bytes_per_line,
        };

        for line in self.data.chunks_mut(bytes_per_line) {
            MemoryView::render_line(&mut self.memory_editor, ui, address, line, self.data_view);
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
