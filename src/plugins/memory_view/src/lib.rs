#[macro_use]
extern crate prodbg_api;

use prodbg_api::{View, Ui, Service, Reader, Writer, PluginHandler, CViewCallbacks, PDVec2, InputTextFlags, Key, ImGuiStyleVar, EventType, InputTextCallbackData};
use std::str;

const BLOCK_SIZE: usize = 1024;
// ProDBG does not respond to requests with low addresses.
const START_ADDRESS: usize = 0xf0000;

struct MemoryCellEditor {
    address: Option<usize>,
    buf: Vec<u8>,
    should_take_focus: bool, // Needed since we cannot change focus in current frame
    should_set_pos_to_start: bool, // Needed since we cannot change cursor position in next frame
}

impl MemoryCellEditor {
    pub fn new() -> MemoryCellEditor {
        MemoryCellEditor {
            address: None,
            buf: vec!(0; 32),
            should_take_focus: false,
            should_set_pos_to_start: false,
        }
    }

    pub fn change_address(&mut self, address: usize, data: &str) {
        self.address = Some(address);
        (&mut self.buf[0..data.len()]).copy_from_slice(data.as_bytes());
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
            DataView::Hex(DataSize::TwoBytes) => {format_buffer!(u16, 2, "{:02x}");}
            DataView::Hex(DataSize::FourBytes) => {format_buffer!(u32, 4, "{:02x}");}
            DataView::Hex(DataSize::EightBytes) => {format_buffer!(u64, 8, "{:02x}");}
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
    memory_editor: MemoryCellEditor,
    memory_request: Option<(usize, usize)>,
    data_view: DataView
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
            ui.set_keyboard_focus_here(0);
            editor.should_take_focus = false;
        }
        ui.push_item_width(width);
        let flags = InputTextFlags::CharsHexadecimal as i32|InputTextFlags::EnterReturnsTrue as i32|InputTextFlags::NoHorizontalScroll as i32|InputTextFlags::AlwaysInsertMode as i32|InputTextFlags::CallbackAlways as i32;
        let mut should_set_pos_to_start = editor.should_set_pos_to_start;
        {
            let callback = |mut data: InputTextCallbackData| {
                if should_set_pos_to_start {
                    data.set_cursor_pos(0);
                    should_set_pos_to_start = false;
                }
            };
            if ui.input_text("##data", &mut editor.buf, flags, Some(&callback)) {
                let text = String::from_utf8(editor.buf.clone()).unwrap();
                new_value = Some(u8::from_str_radix(&text[0..2], 16).unwrap());
                editor.set_inactive();
            }
        }
        editor.should_set_pos_to_start = should_set_pos_to_start;
        ui.pop_item_width();
        ui.pop_style_var(1);
        return new_value;
    }

    fn render_unit(ui: &mut Ui, unit: &[u8], editor: &mut MemoryCellEditor, address: usize, view: DataView) {
        let text = view.format(unit);
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

    fn render_line(editor: &mut MemoryCellEditor, ui: &mut Ui, address: usize, data: &mut [u8], view: DataView) {
        //TODO: Hide editor when user clicks somewhere else
        Self::render_address(ui, address);
        ui.same_line(0, -1);
        let bytes_per_unit = view.byte_count();
        let mut cur_address = address;
        for unit in data.chunks_mut(bytes_per_unit as usize) {
            ui.same_line(0, -1);
//            if editor.address == Some(cur_address) {
//                if let Some(new_value) = Self::render_editor(ui, editor) {
//                    *byte = new_value;
//                    // TODO: send change to ProDBG
//                }
//            } else {
                Self::render_unit(ui, unit, editor, cur_address, view);
//            }
            cur_address += bytes_per_unit as usize;
        }
        Self::render_ansi_string(ui, data);
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
                            self.data_view = DataView::Integer(DataSize::$size, false);
                            println!("Changed to {:?}", self.data_view);
                        }
                        if ui.menu_item("Signed", false, true) {
                            self.data_view = DataView::Integer(DataSize::$size, true);
                            println!("Changed to {:?}", self.data_view);
                        }
                        ui.end_menu();
                    })+
                    ui.end_menu();
                }
            };
            ($variant:ident, $name:expr => $($size:ident),+) => {
                if ui.begin_menu($name, true) {
                    $(if ui.menu_item(DataSize::$size.as_str(), false, true) {
                        self.data_view = DataView::$variant(DataSize::$size);
                        println!("Changed to {:?}", self.data_view);
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
            memory_editor: MemoryCellEditor::new(),
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
            Self::render_line(&mut self.memory_editor, ui, address, line, self.data_view);
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
