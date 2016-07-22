#[macro_use]
extern crate prodbg_api;

mod number_view;
mod hex_editor;
mod char_editor;
mod ascii_editor;
mod address_editor;
mod helper;

use prodbg_api::{View, Ui, Service, Reader, Writer, PluginHandler, CViewCallbacks, PDVec2, ImGuiStyleVar, EventType, ImGuiCol, Color, ReadStatus, Key};
use prodbg_api::PDUIWINDOWFLAGS_HORIZONTALSCROLLBAR;
use std::str;
use number_view::{NumberView, NumberRepresentation, Endianness};
use hex_editor::HexEditor;
use ascii_editor::AsciiEditor;
use address_editor::AddressEditor;
use helper::get_text_cursor_index;
use std::slice::ChunksMut;

const START_ADDRESS: usize = 0xf0000;
const CHARS_PER_ADDRESS: usize = 10;
const TABLE_SPACING: &'static str = "  ";
const COLUMNS_SPACING: &'static str = " ";
// TODO: change to Color when `const fn` is in stable Rust
const CHANGED_DATA_COLOR: u32 = 0xff0000ff;

struct Chunks<'a> {
    cur_address: usize,
    data_address: usize,
    size: usize,
    data: ChunksMut<'a, u8>,
}

impl<'a> Chunks<'a> {
    pub fn new(start_address: usize, data_address: usize, size: usize, data: &'a mut [u8]) -> Chunks<'a> {
        let offset = if data_address > start_address {
            (size - (data_address - start_address) % size) % size
        } else {
            (start_address - data_address) % size
        };
        let iter = if offset < data.len() {
            data[offset..].chunks_mut(size)
        } else {
            [].chunks_mut(size)
        };
        Chunks {
            cur_address: start_address,
            data_address: data_address + offset,
            size: size,
            data: iter,
        }
    }

    pub fn next(&mut self) -> &mut [u8] {
        let res = if self.cur_address < self.data_address {
            &mut []
        } else {
            match self.data.next() {
                Some(res) => res,
                _ => &mut []
            }
        };
        self.cur_address += self.size;
        res
    }
}

/// Enum that acts as cursor for current memory editor.
enum Editor {
    /// Number area is edited right now. `HexEditor` structure contains inner data about focusing
    /// and exact cursor position
    Hex(HexEditor),
    /// Text area is edited right now. `AsciiEditor` structure contains inner data about focusing
    /// and exact cursor position
    Text(AsciiEditor),
    /// Memory is not edited right now
    None,
}

impl Editor {
    pub fn text(&mut self) -> Option<&mut AsciiEditor> {
        match self {
            &mut Editor::Text(ref mut e) => Some(e),
            _ => None,
        }
    }

    pub fn hex(&mut self) -> Option<&mut HexEditor> {
        match self {
            &mut Editor::Hex(ref mut e) => Some(e),
            _ => None,
        }
    }

    pub fn decrease_address(&mut self, delta: usize) {
        match self {
            &mut Editor::Text(ref mut e) => e.address = e.address.saturating_sub(delta),
            &mut Editor::Hex(ref mut e) => e.address = e.address.saturating_sub(delta),
            _ => {}
        }
    }

    pub fn increase_address(&mut self, delta: usize) {
        match self {
            &mut Editor::Text(ref mut e) => e.address = e.address.saturating_add(delta),
            &mut Editor::Hex(ref mut e) => e.address = e.address.saturating_add(delta),
            _ => {}
        }
    }

    /// Returns memory address edited right now, if any.
    pub fn get_address(&self) -> Option<usize> {
        match self {
            &Editor::Text(ref e) => Some(e.address),
            &Editor::Hex(ref e) => Some(e.address),
            _ => None,
        }
    }

    pub fn set_address(&mut self, address: usize) {
        match self {
            &mut Editor::Text(ref mut e) => e.address = address,
            &mut Editor::Hex(ref mut e) => {
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
    /// Address of first byte of `data` and `prev_data`
    data_start_address: usize,
    /// Current state of memory
    data: Vec<u8>,
    /// Snapshotted state of memory
    prev_data: Vec<u8>,
    /// Memory that was requested but not yet received
    memory_request: Option<(usize, usize)>,
    /// Set to force memory update
    should_update_memory: bool,
    /// Number of columns shown (if number view is on) or number of bytes shown
    columns: usize,
    /// Cursor of memory editor
    memory_editor: Editor,
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

    fn render_line(editor: &mut Editor, ui: &mut Ui, address: usize, data: &mut [u8], prev_data: &[u8], view: Option<NumberView>, writer: &mut Writer, columns: usize, text_shown: bool) -> Option<Editor> {
        //TODO: Hide editor when user clicks somewhere else
        MemoryView::render_address(ui, address);

        let mut new_data = None;
        let mut res = None;
        if let Some(view) = view {
            ui.same_line(0, -1);
            ui.text(TABLE_SPACING);
            let (hex_editor, hex_data) = MemoryView::render_numbers(ui, editor.hex(), address, data, prev_data, view, columns);
            res = res.or(hex_editor.map(|editor| Editor::Hex(editor)));
            new_data = new_data.or(hex_data);
        }
        if text_shown {
            ui.same_line(0, -1);
            ui.text(TABLE_SPACING);
            let line_len = columns * match view {
                Some(ref v) => v.size.byte_count(),
                _ => 1,
            };
            let (ascii_editor, ascii_data) = MemoryView::render_ascii_string(ui, address, data, prev_data, line_len, editor.text());
            res = res.or_else(|| ascii_editor.map(|editor| Editor::Text(editor)));
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
            self.memory_editor = Editor::None;
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
            let new_address = self.start_address.get_value();
            self.memory_editor.set_address(new_address);
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
                },
                // TODO: change this event to one or several that correspond to executing code
                et if et == EventType::SetBreakpoint as i32 => {
                    println!("Breakpoint moved");
                    self.process_step();
                }
                _ => {}//println!("Got unknown event type: {:?}", event_type)}
            }
        }
    }

    fn get_memory_intersection(first_start: usize, first_len: usize, second_start: usize, second_len: usize) -> Option<(usize, usize)> {
        let istart = std::cmp::max(first_start, second_start);
        let iend = std::cmp::min(first_start + first_len, second_start + second_len);
        if iend > istart {
            Some((istart, iend - istart))
        } else {
            None
        }
    }

    fn update_memory(&mut self, reader: &mut Reader) -> Result<(), ReadStatus> {
        let address = try!(reader.find_u64("address")) as usize;
        let data = try!(reader.find_data("data"));
        println!("Got {} bytes of data at {:#x}", data.len(), address);
        // TODO: set limits here. Do not copy more bytes than were requested.
        self.data.resize(data.len(), 0);
        (&mut self.data).copy_from_slice(data);

        if self.data_start_address == address {
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
        } else {
            if let Some((start, len)) = MemoryView::get_memory_intersection(self.data_start_address, self.prev_data.len(), address, data.len()) {
                let mut common = Vec::with_capacity(len);
                let pdstart = start - self.data_start_address;
                common.extend_from_slice(&self.prev_data[pdstart..pdstart+len]);
                self.prev_data.resize(data.len(), 0);
                self.prev_data.copy_from_slice(data);
                let ndstart = start - address;
                (&mut self.prev_data[ndstart..ndstart + len]).copy_from_slice(&common);
            } else {
                self.prev_data.resize(data.len(), 0);
                self.prev_data.copy_from_slice(data);
            }
        }

        self.data_start_address = address;
        // Since we cannot say if this is data we requested, we will always assume this to be true
        self.memory_request = None;
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

    fn handle_scroll_keys(&mut self, ui: &Ui, bytes_per_line: usize, lines_on_screen: usize) {
        if ui.is_key_pressed(Key::Up, true) {
            self.memory_editor.decrease_address(bytes_per_line);
        }
        if ui.is_key_pressed(Key::Down, true) {
            self.memory_editor.increase_address(bytes_per_line);
        }
        if ui.is_key_pressed(Key::PageUp, true) {
            self.memory_editor.decrease_address(bytes_per_line * lines_on_screen);
        }
        if ui.is_key_pressed(Key::PageDown, true) {
            self.memory_editor.increase_address(bytes_per_line * lines_on_screen);
        }
        let wheel = ui.get_mouse_wheel();
        if wheel > 0.0 {
            self.memory_editor.decrease_address(bytes_per_line);
        }
        if wheel < 0.0 {
            self.memory_editor.increase_address(bytes_per_line);
        }
    }

    fn move_memory_to_cursor(&mut self, bytes_per_line: usize, lines_on_screen: usize) {
        if let Some(address) = self.memory_editor.get_address() {
            let start_address = self.start_address.get_value();
            if address < start_address {
                let lines_needed = (start_address - address + bytes_per_line - 1) / bytes_per_line;
                self.start_address.set_value(start_address.saturating_sub(lines_needed * bytes_per_line));
            }
            let last_address = self.start_address.get_value().saturating_add(bytes_per_line * lines_on_screen);
            if address >= last_address {
                let lines_needed = (address - last_address) / bytes_per_line + 1;
                self.start_address.set_value(start_address.saturating_add(lines_needed * bytes_per_line));
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

        let mut address = self.start_address.get_value();
        let mut next_editor = None;
        {
            let mut lines = Chunks::new(self.start_address.get_value(), self.data_start_address, bytes_per_line, &mut self.data);
            let mut prev_lines = Chunks::new(self.start_address.get_value(), self.data_start_address, bytes_per_line, &mut self.prev_data);
            for _ in 0..lines_needed {
                let line = lines.next();
                let prev_line = prev_lines.next();
                next_editor = next_editor.or(
                    MemoryView::render_line(&mut self.memory_editor, ui, address, line, prev_line,
                                            self.number_view, writer, columns, self.text_shown)
                );
                address += bytes_per_line;
            }
        }

        ui.end_child();
        ui.pop_style_var(1);

        if let Some(editor) = next_editor {
            self.memory_editor = editor;
        }
        self.handle_scroll_keys(ui, bytes_per_line, lines_needed);
        self.move_memory_to_cursor(bytes_per_line, lines_needed);
    }

    fn process_memory_request(&mut self, writer: &mut Writer) {
        let (start, size) = self.memory_request.unwrap_or((self.data_start_address, self.data.len()));
        let (_, len) = MemoryView::get_memory_intersection(start, size, self.start_address.get_value(), self.bytes_needed).unwrap_or((0, 0));
        if len < self.bytes_needed {
            // Amount of data we can show is less than needed
            self.should_update_memory = true;
        }
        if self.should_update_memory {
            let address = self.start_address.get_value();
            println!("Requesting {} bytes of data at {:#x}", self.bytes_needed, address);
            writer.event_begin(EventType::GetMemory as u16);
            writer.write_u64("address_start", address as u64);
            writer.write_u64("size", self.bytes_needed as u64);
            writer.event_end();
            self.should_update_memory = false;
            self.memory_request = Some((address, self.bytes_needed));
        }
    }
}

impl View for MemoryView {
    fn new(_: &Ui, _: &Service) -> Self {
        MemoryView {
            start_address: AddressEditor::new(START_ADDRESS),
            data_start_address: 0,
            data: Vec::new(),
            prev_data: Vec::new(),
            should_update_memory: false,
            memory_request: None,
            bytes_needed: 0,
            columns: 0,
            memory_editor: Editor::None,
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
