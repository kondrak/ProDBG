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
pub enum NumberSize {
    OneByte,
    TwoBytes,
    FourBytes,
    EightBytes,
}

impl NumberSize {
    /// String representation of this `NumberSize`
    pub fn as_str(&self) -> &'static str {
        match *self {
            NumberSize::OneByte => "1 byte",
            NumberSize::TwoBytes => "2 bytes",
            NumberSize::FourBytes => "4 bytes",
            NumberSize::EightBytes => "8 bytes",
        }
    }

    /// Number of bytes represented by this `NumberSize`
    pub fn byte_count(&self) -> usize {
        match *self {
            NumberSize::OneByte => 1,
            NumberSize::TwoBytes => 2,
            NumberSize::FourBytes => 4,
            NumberSize::EightBytes => 8,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum NumberRepresentation {
    Hex = 0,
    UnsignedDecimal,
    SignedDecimal,
    Float,
}

static NUMBER_REPRESENTATION_STRINGS: [&'static str; 4] = ["Hex", "Signed decimal", "Unsigned decimal", "Float"];
impl NumberRepresentation {
    pub fn as_usize(&self) -> usize {
        *self as usize
    }

    pub fn as_strings() -> &'static [&'static str] {
        &NUMBER_REPRESENTATION_STRINGS
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NumberView {
    representation: NumberRepresentation,
    size: NumberSize,
    // TODO: add endianness
}

impl NumberView {
    /// Maximum number of characters needed to show number
    // TODO: change to calculation from MAX/MIN when `const fn` is in stable Rust
    pub fn maximum_chars_needed(&self) -> usize {
        match self.representation {
            NumberRepresentation::Hex => self.size.byte_count() * 2,
            NumberRepresentation::UnsignedDecimal => match self.size {
                NumberSize::OneByte => 3,
                NumberSize::TwoBytes => 5,
                NumberSize::FourBytes => 10,
                NumberSize::EightBytes => 20,
            },
            NumberRepresentation::SignedDecimal => match self.size {
                NumberSize::TwoBytes => 6,
                NumberSize::OneByte => 4,
                NumberSize::FourBytes => 11,
                NumberSize::EightBytes => 20,
            },
            // TODO: pick a proper representation for floats
            NumberRepresentation::Float => self.size.byte_count() * 2,
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
        match self.representation {
            NumberRepresentation::Hex => match self.size {
                NumberSize::OneByte => {format_buffer!(u8, 1, "{:02x}");}
                NumberSize::TwoBytes => {format_buffer!(u16, 2, "{:04x}");}
                NumberSize::FourBytes => {format_buffer!(u32, 4, "{:08x}");}
                NumberSize::EightBytes => {format_buffer!(u64, 8, "{:016x}");}
            },
            NumberRepresentation::UnsignedDecimal => match self.size {
                NumberSize::OneByte => {format_buffer!(u8, 1, "{:3}");}
                NumberSize::TwoBytes => {format_buffer!(u16, 2, "{:5}");}
                NumberSize::FourBytes => {format_buffer!(u32, 4, "{:10}");}
                NumberSize::EightBytes => {format_buffer!(u64, 8, "{:20}");}
            },
            NumberRepresentation::SignedDecimal => match self.size {
                NumberSize::OneByte => {format_buffer!(i8, 1, "{:4}");}
                NumberSize::TwoBytes => {format_buffer!(i16, 2, "{:6}");}
                NumberSize::FourBytes => {format_buffer!(i32, 4, "{:11}");}
                NumberSize::EightBytes => {format_buffer!(i64, 8, "{:20}");}
            },
            NumberRepresentation::Float => match self.size {
                NumberSize::FourBytes => {format_buffer!(f32, 4, "{}");}
                NumberSize::EightBytes => {format_buffer!(f64, 8, "{}");}
                // Should never be available to pick through user interface
                _ => return "Error".to_owned()
            },
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
    number_view: NumberView
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

    fn render_number(ui: &mut Ui, slice: &[u8], editor: &mut HexMemoryEditor, address: usize, view: NumberView) {
        let text = view.format(slice);
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

    fn render_line(editor: &mut HexMemoryEditor, ui: &mut Ui, address: usize, data: &mut [u8], view: NumberView) {
        //TODO: Hide editor when user clicks somewhere else
        MemoryView::render_address(ui, address);
        ui.same_line(0, -1);
        let bytes_per_unit = view.size.byte_count();
        let mut cur_address = address;
        for unit in data.chunks_mut(bytes_per_unit) {
            ui.same_line(0, -1);
            if editor.address == Some(cur_address) {
                if MemoryView::render_memory_editor(ui, editor, unit) {
                    // TODO: send change to ProDBG
                    editor.text = view.format(unit);
                }
            } else {
                MemoryView::render_number(ui, unit, editor, cur_address, view);
            }
            cur_address += bytes_per_unit as usize;
        }
        MemoryView::render_ansi_string(ui, data);
    }

    fn change_number_view(&mut self, view: NumberView) {
        self.number_view = view;
        self.memory_editor.set_inactive();
        self.memory_editor.digit_count = view.size.byte_count() * 2;
    }

    fn render_number_view_picker(&mut self, ui: &mut Ui) {
        let mut current_item = self.number_view.representation.as_usize();
        let strings = NumberRepresentation::as_strings();
        if ui.combo("##number_representation", &mut current_item, strings, strings.len(), strings.len()) {
            println!("Changed to {}", current_item);
        }
//        if ui.button("Something", None) {
//            ui.open_popup("##data_view");
//        }
//        macro_rules! data_view_menu {
//            (Integer, $name:expr => $($size:ident),+) => {
//                if ui.begin_menu($name, true) {
//                    $(if ui.begin_menu(NumberSize::$size.as_str(), true) {
//                        if ui.menu_item("Unsigned", false, true) {
//                            self.change_data_view(DataView::Integer(NumberSize::$size, false));
//                        }
//                        if ui.menu_item("Signed", false, true) {
//                            self.change_data_view(DataView::Integer(NumberSize::$size, true));
//                        }
//                        ui.end_menu();
//                    })+
//                    ui.end_menu();
//                }
//            };
//            ($variant:ident, $name:expr => $($size:ident),+) => {
//                if ui.begin_menu($name, true) {
//                    $(if ui.menu_item(NumberSize::$size.as_str(), false, true) {
//                        self.change_data_view(DataView::$variant(NumberSize::$size));
//                    })+
//                    ui.end_menu();
//                }
//            };
//        }
//        if ui.begin_popup("##data_view") {
//            data_view_menu!(Hex, "Hex" => OneByte, TwoBytes, FourBytes, EightBytes);
//            data_view_menu!(Integer, "Integer" => OneByte, TwoBytes, FourBytes, EightBytes);
//            data_view_menu!(Float, "Float" => FourBytes, EightBytes);
//            ui.end_popup();
//        }
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
        self.render_number_view_picker(ui);
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
            number_view: NumberView {representation: NumberRepresentation::Hex, size: NumberSize::OneByte}
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
                let chars_left = (screen_left / glyph_size) as usize;
                let unit_size = self.number_view.size.byte_count();
                // Number of chars we need to draw one unit
                let chars_per_unit = self.number_view.maximum_chars_needed() + 1 + unit_size;
                if chars_left > chars_per_unit {
                    (chars_left / chars_per_unit * unit_size)
                } else {
                    unit_size
                }
            },
            _ => self.bytes_per_line,
        };

        for line in self.data.chunks_mut(bytes_per_line) {
            MemoryView::render_line(&mut self.memory_editor, ui, address, line, self.number_view);
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
