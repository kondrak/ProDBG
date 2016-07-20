#[macro_use]
extern crate prodbg_api;

mod number_view;
mod hex_editor;
mod char_editor;
mod ascii_editor;
mod helper;

use prodbg_api::{View, Ui, Service, Reader, Writer, PluginHandler, CViewCallbacks, PDVec2, InputTextFlags, ImGuiStyleVar, EventType, ImGuiCol, Color, ReadStatus};
use prodbg_api::PDUIWINDOWFLAGS_HORIZONTALSCROLLBAR;
use std::str;
use number_view::{NumberView, NumberRepresentation, NumberSize, Endianness};
use hex_editor::HexEditor;
use ascii_editor::AsciiEditor;
use helper::get_text_cursor_index;

const START_ADDRESS: usize = 0xf0000;
const TABLE_SPACING: &'static str = "  ";
const COLUMNS_SPACING: &'static str = " ";
// TODO: change to Color when `const fn` is in stable Rust
const CHANGED_DATA_COLOR: u32 = 0xff0000ff;

struct InputText {
    // TODO: What buffer do we really need for address?
    buf: [u8; 20],
    value: usize,
}

impl InputText {
    pub fn new(value: usize) -> InputText {
        let mut res = InputText {
            buf: [0; 20],
            value: 0,
        };
        res.set_value(value);
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

enum Editor {
    Hex(HexEditor),
    Text(AsciiEditor),
    None,
}

impl Editor {
    fn text(&mut self) -> Option<&mut AsciiEditor> {
        match self {
            &mut Editor::Text(ref mut e) => Some(e),
            _ => None,
        }
    }
}

const COLUMNS_TEXT_VARIANTS: [&'static str; 9] = ["Fit width", "1 column", "2 columns", "4 columns", "8 columns", "16 columns", "32 columns", "64 columns", "128 columns"];
const COLUMNS_NUM_VARIANTS: [usize; 9] = [0, 1, 2, 4, 8, 16, 32, 64, 128];
struct MemoryView {
    data: Vec<u8>,
    prev_data: Vec<u8>,
    bytes_requested: usize,
    start_address: InputText,
    columns: usize,
    chars_per_address: usize,
    memory_editor: Editor,
    memory_request: Option<(usize, usize)>,
    number_view: NumberView,
    text_shown: bool,
}

impl MemoryView {
    fn render_address(ui: &mut Ui, address: usize) {
        ui.text(&format!("{:#010x}", address));
    }

    fn render_number(ui: &mut Ui, text: &str) -> Option<usize> {
        ui.text(text);
        if ui.is_item_hovered() && ui.is_mouse_clicked(0, false) {
            return Some(get_text_cursor_index(ui, text.len()));
        } else {
            return None;
        }
    }

    fn render_inaccessible_memory(ui: &mut Ui, char_count: usize) {
        let mut text = String::with_capacity(char_count);
        for _ in 0..char_count {
            text.push('?');
        }
        ui.text(&text);
    }

    fn render_ascii_string(ui: &mut Ui, mut address: usize, data: &mut [u8], prev_data: &[u8], char_count: usize, mut editor: Option<&mut AsciiEditor>) -> (Option<AsciiEditor>, Option<(usize, usize)>) {
        let mut bytes = data.iter_mut();
        let mut prev_bytes = prev_data.iter();
        let mut next_editor = None;
        let mut changed_data = None;
        for _ in 0..char_count {
            let mut cur_char = bytes.next();
            let prev_char = prev_bytes.next();
            let mut is_marked = false;
            if let Some(ref cur) = cur_char {
                if let Some(ref prev) = prev_char {
                    is_marked = cur != prev;
                }
            }
            if is_marked {
                ui.push_style_color(ImGuiCol::Text, Color::from_u32(CHANGED_DATA_COLOR));
            }
            let mut is_editor = false;
            ui.same_line(0, -1);
            if let Some(ref mut c) = cur_char {
                if let Some(ref mut e) = editor {
                    if e.address == address {
                        is_editor = true;
                        let (pos, has_changed) = e.render(ui, c);
                        if has_changed {
                            changed_data = Some((address, 1));
                        }
                        next_editor = next_editor.or(pos.map(|address| AsciiEditor::new(address)));
                    }
                }
            }
            if !is_editor {
                match cur_char {
                    Some(byte) => {
                        match *byte {
                            32...127 => ui.text( unsafe { std::str::from_utf8_unchecked( & [ * byte]) }),
                            _ => ui.text("."),
                        }
                        if ui.is_item_hovered() && ui.is_mouse_clicked(0, false) {
                            next_editor = next_editor.or_else(|| Some(AsciiEditor::new(address)));
                        }
                    },
                    None => ui.text("?"),
                };
            }
            if is_marked {
                ui.pop_style_color(1);
            }
            address += 1;
        }
        (next_editor, changed_data)
    }

    fn set_memory(writer: &mut Writer, address: usize, data: &[u8]) {
        writer.event_begin(EventType::UpdateMemory as u16);
        writer.write_u64("address", address as u64);
        writer.write_data("data", data);
        writer.event_end();
    }

    fn render_line(editor: &mut Editor, ui: &mut Ui, address: usize, data: &mut [u8], prev_data: &[u8], view: NumberView, writer: &mut Writer, columns: usize, text_shown: bool) -> Option<Editor> {
        //TODO: Hide editor when user clicks somewhere else
        MemoryView::render_address(ui, address);
        ui.same_line(0, -1);
        ui.text(TABLE_SPACING);
        ui.same_line(0, -1);

        let bytes_per_unit = view.size.byte_count();
        let mut cur_address = address;
        let mut res = None;
        let mut new_data = None;
        {
            let mut data_chunks = data.chunks_mut(bytes_per_unit);
            let mut prev_data_chunks = prev_data.chunks(bytes_per_unit);
            for column in 0..columns {
                ui.same_line(0, -1);
                match data_chunks.next() {
                    Some(ref mut unit) if unit.len() == bytes_per_unit => {
                        let has_changed = match prev_data_chunks.next() {
                            Some(ref prev_unit) if prev_unit.len() == bytes_per_unit => unit != prev_unit,
                            _ => false,
                        };
                        if has_changed {
                            ui.push_style_color(ImGuiCol::Text, Color::from_u32(CHANGED_DATA_COLOR));
                        }
                        let mut is_editor = false;
                        if let &mut Editor::Hex(ref mut e) = editor {
                            if e.is_at_address(cur_address) {
                                let (np, data_edited) = e.render(ui, *unit);
                                res = res.or(np.map(|(address, cursor)|
                                    Editor::Hex(HexEditor::new(address, cursor, view))
                                ));
                                if data_edited {
                                    new_data = Some((cur_address, bytes_per_unit));
                                }
                                is_editor = true;
                            }
                        }
                        if !is_editor {
                            if let Some(index) = MemoryView::render_number(ui, &view.format(*unit)) {
                                if view.representation == NumberRepresentation::Hex {
                                    res = Some(Editor::Hex(HexEditor::new(cur_address, index, view)));
                                }
                            }
                        }
                        if has_changed {
                            ui.pop_style_color(1);
                        }
                    },
                    _ => {
                        MemoryView::render_inaccessible_memory(ui, view.maximum_chars_needed());
                    }
                }
                if column < columns - 1 {
                    ui.same_line(0, -1);
                    ui.text(COLUMNS_SPACING);
                }
                cur_address += bytes_per_unit as usize;
            }
        }
        if text_shown {
            ui.same_line(0, -1);
            ui.text(TABLE_SPACING);
            let ascii_res = MemoryView::render_ascii_string(ui, address, data, prev_data, columns * bytes_per_unit, editor.text());
            if let Some(next_editor) = ascii_res.0 {
                res = Some(Editor::Text(next_editor));
            }
            new_data = new_data.or(ascii_res.1);
        }
        if let Some((abs_address, size)) = new_data {
            let offset = abs_address - address;
            MemoryView::set_memory(writer, abs_address, &data[offset..offset+size]);
        }
        return res;
    }

    fn change_number_view(&mut self, view: NumberView) {
        self.number_view = view;
        self.memory_editor = Editor::None;
    }

    fn render_number_view_picker(&mut self, ui: &mut Ui) {
        let mut view = self.number_view;
        let mut view_is_changed = false;
        let mut current_item;

        let strings = NumberRepresentation::names();
        current_item = view.representation.as_usize();
        // TODO: should we calculate needed width from strings?
        ui.push_item_width(200.0);
        if ui.combo("##number_representation", &mut current_item, strings, strings.len(), strings.len()) {
            view.change_representation(NumberRepresentation::from_usize(current_item));
            view_is_changed = true;
        }
        ui.pop_item_width();

        let available_sizes = view.representation.get_avaialable_sizes();
        let strings: Vec<&str> = available_sizes.iter().map(|size| size.as_str()).collect();
        current_item = available_sizes.iter().position(|x| *x == view.size).unwrap_or(0);
        ui.same_line(0, -1);
        ui.push_item_width(100.0);
        if ui.combo("##number_size", &mut current_item, &strings, available_sizes.len(), available_sizes.len()) {
            view.size = *available_sizes.get(current_item).unwrap_or_else(|| available_sizes.first().unwrap());
            view_is_changed = true;
        }
        ui.pop_item_width();

        let strings = Endianness::names();
        current_item = view.endianness.as_usize();
        ui.same_line(0, -1);
        ui.push_item_width(200.0);
        if ui.combo("##endianness", &mut current_item, strings, strings.len(), strings.len()) {
            view.endianness = Endianness::from_usize(current_item);
            view_is_changed = true;
        }
        ui.pop_item_width();

        if view_is_changed {
            self.change_number_view(view);
        }
    }

    fn render_columns_picker(&mut self, ui: &mut Ui) {
        ui.push_item_width(200.0);
        let mut cur_item = COLUMNS_NUM_VARIANTS.iter().position(|&x| x == self.columns).unwrap_or(0);
        if ui.combo("##byte_per_line", &mut cur_item, &COLUMNS_TEXT_VARIANTS, COLUMNS_TEXT_VARIANTS.len(), COLUMNS_TEXT_VARIANTS.len()) {
            self.columns = COLUMNS_NUM_VARIANTS.get(cur_item).map(|x| *x).unwrap_or(0);
        }
        ui.pop_item_width();
    }

    fn render_header(&mut self, ui: &mut Ui) {
        ui.text("0x");
        ui.same_line(0, 0);
        ui.push_style_var_vec(ImGuiStyleVar::FramePadding, PDVec2{x: 1.0, y: 0.0});
        ui.push_item_width(ui.calc_text_size("00000000", 0).0 + 2.0);
        if self.start_address.render(ui) {
            self.memory_request = Some((self.start_address.get_value(), self.bytes_requested));
        }
        ui.pop_item_width();
        ui.pop_style_var(1);
        ui.same_line(0, -1);
        self.render_number_view_picker(ui);
        ui.same_line(0, -1);
        self.render_columns_picker(ui);
        ui.same_line(0, -1);
        ui.checkbox("Show text", &mut self.text_shown);
    }

    fn process_step(&mut self) {
        std::mem::swap(&mut self.data, &mut self.prev_data);
        self.memory_request = Some((self.start_address.get_value(), self.bytes_requested));
    }

    fn process_events(&mut self, reader: &mut Reader) {
        for event_type in reader.get_events() {
            match event_type {
                et if et == EventType::SetMemory as i32 => {
                    if let Err(e) = self.update_memory(reader) {
                        println!("Could not update memory: {:?}", e);
                    }
                },
                // TODO: change this event to one or several that correspond to executing code
                et if et == EventType::SetBreakpoint as i32 => {
                    self.process_step();
                }
                _ => {}//println!("Got unknown event type: {:?}", event_type)}
            }
        }
    }

    fn update_memory(&mut self, reader: &mut Reader) -> Result<(), ReadStatus> {
        let address = try!(reader.find_u64("address"));
        let data = try!(reader.find_data("data"));
        self.start_address.set_value(address as usize);
        // TODO: set limits here. Do not copy more bytes than were reqeusted.
        self.data.resize(data.len(), 0);
        (&mut self.data).copy_from_slice(data);
        let prev_data_len = self.prev_data.len();
        if prev_data_len < data.len() {
            // Do not rewrite stored data, only append data that was missing. Needed for next
            // situation:
            // * user changes data: prev_data and data differ;
            // * user extends window of MemoryView
            // * `data` of bigger size arrives and replaces `self.data`
            // In this situation we cannot replace `prev_data` since it will lose
            // information about changes that user did before. Also we cannot leave
            // `self.prev_data` unchanged because user will not see changes that he makes in
            // newly added piece of memory. The only thing we can do is to add newly added
            // piece of memory to `prev_data`.
            self.prev_data.extend(&data[prev_data_len..]);
        } else {
            self.prev_data.truncate(data.len());
        }
        Ok(())
    }

    /// Returns maximum amount of bytes that could be rendered within window width
    /// Minimum number of columns reported is 1.
    fn get_columns_from_width(&self, ui: &Ui) -> usize {
        // TODO: ImGui reports inaccurate glyph size. Find a better way to find chars_in_screen.
        let glyph_size = ui.calc_text_size("ff", 0).0 / 2.0;
        let chars_in_screen = (ui.get_window_size().0 / glyph_size) as usize;
        let chars_left = if self.text_shown {
            chars_in_screen.saturating_sub(2 * TABLE_SPACING.len() + self.chars_per_address)
        } else {
            chars_in_screen.saturating_sub(TABLE_SPACING.len() + self.chars_per_address)
        };
        let chars_per_unit = if self.text_shown {
            // Number of chars we need to draw one unit: number view, space, text view
            self.number_view.maximum_chars_needed() + COLUMNS_SPACING.len() + self.number_view.size.byte_count()
        } else {
            self.number_view.maximum_chars_needed() + COLUMNS_SPACING.len()
        };
        return std::cmp::max(chars_left / chars_per_unit, 1);
    }
}

impl View for MemoryView {
    fn new(_: &Ui, _: &Service) -> Self {
        let view = NumberView {
            representation: NumberRepresentation::Hex,
            size: NumberSize::OneByte,
            endianness: Endianness::default(),
        };
        MemoryView {
            data: Vec::new(),
            prev_data: Vec::new(),
            start_address: InputText::new(START_ADDRESS),
            bytes_requested: 0,
            columns: 0,
            chars_per_address: 10,
            memory_editor: Editor::None,
            memory_request: None,
            number_view: view,
            text_shown: true,
        }
    }

    fn update(&mut self, ui: &mut Ui, reader: &mut Reader, writer: &mut Writer) {
        self.process_events(reader);
        self.render_header(ui);
        let mut address = self.start_address.value;
        let columns = match self.columns {
            0 => self.get_columns_from_width(ui),
            x => x,
        };
        let bytes_per_line = columns * self.number_view.size.byte_count();

        ui.push_style_var_vec(ImGuiStyleVar::ItemSpacing, PDVec2 {x: 0.0, y: 0.0});
        let line_height = ui.get_text_line_height_with_spacing();
        let (start, end) = ui.calc_list_clipping(line_height);
        // Strip last line to make sure vertical scrollbar will not appear
        let lines_needed = end.saturating_sub(start + 1);
        let bytes_needed = bytes_per_line * lines_needed;
        if bytes_needed > self.bytes_requested {
            self.memory_request = Some((self.start_address.get_value(), bytes_needed));
        }

        ui.begin_child("##lines", None, false, PDUIWINDOWFLAGS_HORIZONTALSCROLLBAR);

        let mut next_editor_position = None;
        let mut lines = self.data.chunks_mut(bytes_per_line);
        let mut prev_lines = self.prev_data.chunks(bytes_per_line);
        for _ in 0..lines_needed {
            let line = lines.next().unwrap_or(&mut []);
            let prev_line = prev_lines.next().unwrap_or(&[]);
            let next_position = MemoryView::render_line(&mut self.memory_editor, ui, address, line, prev_line, self.number_view, writer, columns, self.text_shown);
            if next_position.is_some() {
                next_editor_position = next_position;
            }
            address += bytes_per_line;
        }

        ui.end_child();
        ui.pop_style_var(1);

        if let Some(editor) = next_editor_position {
            self.memory_editor = editor;
        }

        if let Some((address, size)) = self.memory_request {
            self.bytes_requested = size;
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
