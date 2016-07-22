#[macro_use]
extern crate prodbg_api;

mod number_view;
mod hex_editor;
mod char_editor;
mod ascii_editor;
mod address_editor;
mod helper;
mod memory_chunk;

use prodbg_api::{View, Ui, Service, Reader, Writer, PluginHandler, CViewCallbacks, PDVec2, ImGuiStyleVar, EventType, ImGuiCol, Color, ReadStatus, Key};
use prodbg_api::PDUIWINDOWFLAGS_HORIZONTALSCROLLBAR;
use std::str;
use number_view::{NumberView, NumberRepresentation, Endianness};
use hex_editor::HexEditor;
use ascii_editor::AsciiEditor;
use address_editor::AddressEditor;
use helper::get_text_cursor_index;
use memory_chunk::MemoryChunk;

const START_ADDRESS: usize = 0xf0000;
const CHARS_PER_ADDRESS: usize = 10;
const TABLE_SPACING: &'static str = "  ";
const COLUMNS_SPACING: &'static str = " ";
// TODO: change to Color when `const fn` is in stable Rust
const CHANGED_DATA_COLOR: u32 = 0xff0000ff;
const LINES_PER_SCROLL: usize = 3;

#[derive(Clone)]
enum Cursor {
    /// Number area is edited right now. `HexEditor` structure contains inner data about focusing
    /// and exact cursor position
    Number(HexEditor),
    /// Text area is edited right now. `AsciiEditor` structure contains inner data about focusing
    /// and exact cursor position
    Text(AsciiEditor),
    /// Memory is not edited right now
    None,
}

impl Cursor {
    pub fn text(&mut self) -> Option<&mut AsciiEditor> {
        match self {
            &mut Cursor::Text(ref mut e) => Some(e),
            _ => None,
        }
    }

    pub fn number(&mut self) -> Option<&mut HexEditor> {
        match self {
            &mut Cursor::Number(ref mut e) => Some(e),
            _ => None,
        }
    }

    pub fn decrease_address(&mut self, delta: usize) {
        match self {
            &mut Cursor::Text(ref mut e) => e.address = e.address.saturating_sub(delta),
            &mut Cursor::Number(ref mut e) => e.address = e.address.saturating_sub(delta),
            _ => {}
        }
    }

    pub fn increase_address(&mut self, delta: usize) {
        match self {
            &mut Cursor::Text(ref mut e) => e.address = e.address.saturating_add(delta),
            &mut Cursor::Number(ref mut e) => e.address = e.address.saturating_add(delta),
            _ => {}
        }
    }

    /// Returns memory address edited right now, if any.
    pub fn get_address(&self) -> Option<usize> {
        match self {
            &Cursor::Text(ref e) => Some(e.address),
            &Cursor::Number(ref e) => Some(e.address),
            _ => None,
        }
    }

    pub fn set_address(&mut self, address: usize) {
        match self {
            &mut Cursor::Text(ref mut e) => e.address = address,
            &mut Cursor::Number(ref mut e) => {
                e.address = address;
                e.cursor = 0;
            },
            _ => {}
        }
    }
}

const COLUMNS_TEXT_VARIANTS: [&'static str; 9] = ["Fit width", "1 column", "2 columns", "4 columns", "8 columns", "16 columns", "32 columns", "64 columns", "128 columns"];
const COLUMNS_NUM_VARIANTS: [usize; 9] = [0, 1, 2, 4, 8, 16, 32, 64, 128];
struct MemoryView {
    /// Address of first byte of memory shown
    start_address: AddressEditor,
    /// Amount of bytes needed to fill one screen
    bytes_needed: usize,
    /// Current state of memory
    data: MemoryChunk,
    /// Snapshotted state of memory
    prev_data: MemoryChunk,
    /// Set to force memory update
    should_update_memory: bool,
    /// Number of columns shown (if number view is on) or number of bytes shown
    columns: usize,
    /// Cursor of memory editor
    cursor: Cursor,
    /// Picked number view
    number_view: Option<NumberView>,
    /// Picked text view (currently on/off since only ascii text view is available)
    text_shown: bool,
}

impl MemoryView {
    fn render_address(ui: &mut Ui, address: usize) {
        ui.text(&format!("{:#0width$x}", address, width = CHARS_PER_ADDRESS));
    }

    fn render_const_number(ui: &mut Ui, text: &str) -> Option<usize> {
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

    fn render_numbers(ui: &mut Ui, mut editor: Option<&mut HexEditor>, address: usize, data: &mut [u8], prev_data: &[u8], view: NumberView, columns: usize) -> (Option<HexEditor>, Option<(usize, usize)>) {
        let bytes_per_unit = view.size.byte_count();
        let mut next_editor = None;
        let mut changed_data = None;
        let mut cur_address = address;
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
                        if let Some(ref mut e) = editor {
                            if e.address == cur_address {
                                let (np, data_edited) = e.render(ui, *unit);
                                next_editor = next_editor.or(np.map(|(address, cursor)|
                                    HexEditor::new(address, cursor, view)
                                ));
                                if data_edited {
                                    changed_data = Some((cur_address, bytes_per_unit));
                                }
                                is_editor = true;
                            }
                        }
                        if !is_editor {
                            if let Some(index) = MemoryView::render_const_number(ui, &view.format(*unit)) {
                                next_editor = next_editor.or(Some(HexEditor::new(cur_address, index, view)));
                            }
                        }
                        if has_changed {
                            ui.pop_style_color(1);
                        }
                    },
                    _ => MemoryView::render_inaccessible_memory(ui, view.maximum_chars_needed())
                }
                if column < columns - 1 {
                    ui.same_line(0, -1);
                    ui.text(COLUMNS_SPACING);
                }
                cur_address += bytes_per_unit as usize;
            }
        }
        (next_editor, changed_data)
    }

    fn render_line(cursor: &mut Cursor, ui: &mut Ui, address: usize, data: &mut [u8], prev_data: &[u8], view: Option<NumberView>, writer: &mut Writer, columns: usize, text_shown: bool) -> Option<Cursor> {
        //TODO: Hide cursor when user clicks somewhere else
        MemoryView::render_address(ui, address);

        let mut new_data = None;
        let mut res = None;
        if let Some(view) = view {
            ui.same_line(0, -1);
            ui.text(TABLE_SPACING);
            let (hex_editor, hex_data) = MemoryView::render_numbers(ui, cursor.number(), address, data, prev_data, view, columns);
            res = res.or(hex_editor.map(|editor| Cursor::Number(editor)));
            new_data = new_data.or(hex_data);
        }
        if text_shown {
            ui.same_line(0, -1);
            ui.text(TABLE_SPACING);
            let line_len = columns * match view {
                Some(ref v) => v.size.byte_count(),
                _ => 1,
            };
            let (ascii_editor, ascii_data) = MemoryView::render_ascii_string(ui, address, data, prev_data, line_len, cursor.text());
            res = res.or_else(|| ascii_editor.map(|editor| Cursor::Text(editor)));
            new_data = new_data.or(ascii_data);
        }
        if let Some((abs_address, size)) = new_data {
            let offset = abs_address - address;
            MemoryView::set_memory(writer, abs_address, &data[offset..offset+size]);
        }
        return res;
    }

    fn render_number_view_picker(&mut self, ui: &mut Ui) {
        let mut view = self.number_view;
        let mut view_is_changed = false;
        let mut current_item;

        let variants = [NumberRepresentation::Hex, NumberRepresentation::UnsignedDecimal,
            NumberRepresentation::SignedDecimal, NumberRepresentation::Float];
        let strings = ["Off", variants[0].as_str(), variants[1].as_str(), variants[2].as_str(), variants[3].as_str()];
        current_item = match view {
            Some(v) => variants.iter().position(|var| *var == v.representation).unwrap_or(0) + 1,
            None => 0,
        };
        // TODO: should we calculate needed width from strings?
        ui.push_item_width(200.0);
        if ui.combo("##number_representation", &mut current_item, &strings, strings.len(), strings.len()) {
            if current_item == 0 {
                view = None;
            } else {
                match view {
                    Some(ref mut v) => v.change_representation(variants[current_item - 1]),
                    None => view = Some(NumberView::default()),
                }

            }
            view_is_changed = true;
        }
        ui.pop_item_width();

        if let Some(ref mut view) = view {
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
        }

        if view_is_changed {
            self.number_view = view;
            self.cursor = Cursor::None;
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
        if self.start_address.render(ui) {
            let new_address = self.start_address.get();
            self.cursor.set_address(new_address);
        }
        ui.same_line(0, -1);
        self.render_number_view_picker(ui);
        ui.same_line(0, -1);
        self.render_columns_picker(ui);
        ui.same_line(0, -1);
        ui.checkbox("Show text", &mut self.text_shown);
    }

    fn process_step(&mut self) {
        std::mem::swap(&mut self.data, &mut self.prev_data);
        self.should_update_memory = true;
    }

    fn process_events(&mut self, reader: &mut Reader) {
        for event_type in reader.get_events() {
            match event_type {
                et if et == EventType::SetMemory as i32 => {
                    if let Err(e) = self.update_memory(reader) {
                        println!("Could not update memory: {:?}", e);
                    }
                }
                et if et == EventType::SetBreakpoint as i32 => {
                    println!("Breakpoint moved");
                    self.process_step();
                }
                _ => {}
            }
        }
    }

    fn update_memory(&mut self, reader: &mut Reader) -> Result<(), ReadStatus> {
        let address = try!(reader.find_u64("address")) as usize;
        let data = try!(reader.find_data("data"));
        println!("Got {} bytes of data at {:#x}", data.len(), address);
        self.data.set_accessible(address, data);
        self.prev_data.extend_accessible(address, data);
        Ok(())
    }

    /// Returns maximum amount of bytes that could be rendered within window width
    /// Minimum number of columns reported is 1.
    fn get_columns_from_width(&self, ui: &Ui) -> usize {
        // TODO: ImGui reports inaccurate glyph size. Find a better way to find chars_in_screen.
        let glyph_size = ui.calc_text_size("ff", 0).0 / 2.0;
        let mut chars_left = (ui.get_window_size().0 / glyph_size) as usize;
        // Number of large columns (for numbers, text)
        let mut large_columns: usize = 0;
        // Number of chars per one rendered column
        let mut chars_per_column = 0;
        if let Some(ref view) = self.number_view {
            large_columns += 1;
            // Every number is fixed number of chars + spacing between them
            chars_per_column += view.maximum_chars_needed() + COLUMNS_SPACING.len();
        }
        if self.text_shown {
            large_columns += 1;
            // One char per byte.
            chars_per_column += match self.number_view {
                Some(ref view) => view.size.byte_count(),
                None => 1
            }
        }
        chars_left = chars_left.saturating_sub(large_columns * TABLE_SPACING.len() + CHARS_PER_ADDRESS);
        if chars_per_column > 0 {
            std::cmp::max(chars_left / chars_per_column, 1)
        } else {
            // Neither number nor text view is shown
            1
        }
    }

    fn get_screen_lines_count(ui: &Ui) -> usize {
        let line_height = ui.get_text_line_height_with_spacing();
        let (start, end) = ui.calc_list_clipping(line_height);
        // Strip last line to make sure vertical scrollbar will not appear
        end.saturating_sub(start + 1)
    }

    fn handle_cursor_move_keys(&mut self, ui: &Ui, bytes_per_line: usize) -> Option<Cursor> {
        if ui.is_key_pressed(Key::Up, true) {
            let mut cursor = self.cursor.clone();
            cursor.decrease_address(bytes_per_line);
            return Some(cursor);
        }
        if ui.is_key_pressed(Key::Down, true) {
            let mut cursor = self.cursor.clone();
            cursor.increase_address(bytes_per_line);
            return Some(cursor);
        }
        None
    }

    fn handle_scroll_keys(&mut self, ui: &Ui, bytes_per_line: usize, lines_on_screen: usize) {
        let address = self.start_address.get();
        let mut new_address = None;
        if ui.is_key_pressed(Key::PageUp, true) {
            new_address = Some(address.saturating_sub(bytes_per_line * lines_on_screen));
        }
        if ui.is_key_pressed(Key::PageDown, true) {
            new_address = Some(address.saturating_add(bytes_per_line * lines_on_screen));
        }
        let wheel = ui.get_mouse_wheel();
        if wheel > 0.0 {
            new_address = Some(address.saturating_sub(bytes_per_line * LINES_PER_SCROLL));
        }
        if wheel < 0.0 {
            new_address = Some(address.saturating_add(bytes_per_line * LINES_PER_SCROLL));
        }
        if let Some(new_address) = new_address {
            self.start_address.set(new_address);
        }
    }

    fn follow_cursor(&mut self, bytes_per_line: usize, lines_on_screen: usize) {
        if let Some(address) = self.cursor.get_address() {
            let start_address = self.start_address.get();
            if address < start_address {
                let lines_needed = (start_address - address + bytes_per_line - 1) / bytes_per_line;
                self.start_address.set(start_address.saturating_sub(lines_needed * bytes_per_line));
            }
            let last_address = self.start_address.get().saturating_add(bytes_per_line * lines_on_screen);
            if address >= last_address {
                let lines_needed = (address - last_address) / bytes_per_line + 1;
                self.start_address.set(start_address.saturating_add(lines_needed * bytes_per_line));
            }
        }
    }

    fn render(&mut self, ui: &mut Ui, writer: &mut Writer) {
        self.render_header(ui);
        let columns = match self.columns {
            0 => self.get_columns_from_width(ui),
            x => x,
        };
        let bytes_per_line = columns * match self.number_view {
            Some(ref view) => view.size.byte_count(),
            None => 1,
        };

        ui.push_style_var_vec(ImGuiStyleVar::ItemSpacing, PDVec2 {x: 0.0, y: 0.0});
        ui.begin_child("##lines", None, false, PDUIWINDOWFLAGS_HORIZONTALSCROLLBAR);

        let lines_needed = MemoryView::get_screen_lines_count(ui);
        self.bytes_needed = bytes_per_line * lines_needed;

        let mut address = self.start_address.get();
        let mut next_cursor = None;
        {
            let mut lines = self.data.chunks_mut(bytes_per_line);
            let mut prev_lines = self.prev_data.chunks_mut(bytes_per_line);
            for _ in 0..lines_needed {
                let line = lines.next().unwrap_or(&mut []);
                let prev_line = prev_lines.next().unwrap_or(&mut []);
                next_cursor = next_cursor.or(
                    MemoryView::render_line(&mut self.cursor, ui, address, line, prev_line,
                                            self.number_view, writer, columns, self.text_shown)
                );
                address += bytes_per_line;
            }
        }

        ui.end_child();
        ui.pop_style_var(1);

        next_cursor = next_cursor.or_else(|| self.handle_cursor_move_keys(ui, bytes_per_line));

        if let Some(cursor) = next_cursor {
            self.cursor = cursor;
            self.follow_cursor(bytes_per_line, lines_needed);
        }
        self.handle_scroll_keys(ui, bytes_per_line, lines_needed);
    }

    fn process_memory_request(&mut self, writer: &mut Writer) {
        let address = self.start_address.get();
        if address != self.data.start() || self.bytes_needed > self.data.len() {
            self.should_update_memory = true;
        }
        if self.should_update_memory && self.bytes_needed > 0 {
            println!("Requesting {} bytes of data at {:#x}", self.bytes_needed, address);
            writer.event_begin(EventType::GetMemory as u16);
            writer.write_u64("address_start", address as u64);
            writer.write_u64("size", self.bytes_needed as u64);
            writer.event_end();
            self.data.transform(address, self.bytes_needed);
            self.prev_data.transform(address, self.bytes_needed);
            self.should_update_memory = false;
        }
    }
}

impl View for MemoryView {
    fn new(_: &Ui, _: &Service) -> Self {
        MemoryView {
            start_address: AddressEditor::new(START_ADDRESS),
            data: MemoryChunk::new(),
            prev_data: MemoryChunk::new(),
            should_update_memory: false,
            bytes_needed: 0,
            columns: 0,
            cursor: Cursor::None,
            number_view: Some(NumberView::default()),
            text_shown: true,
        }
    }

    fn update(&mut self, ui: &mut Ui, reader: &mut Reader, writer: &mut Writer) {
        self.process_events(reader);
        self.render(ui, writer);
        self.process_memory_request(writer);
    }
}

#[no_mangle]
pub fn init_plugin(plugin_handler: &mut PluginHandler) {
    define_view_plugin!(PLUGIN, b"Memory View\0", MemoryView);
    plugin_handler.register_view(&PLUGIN);
}
