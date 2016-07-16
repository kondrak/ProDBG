#[macro_use]
extern crate prodbg_api;

mod number_view;
mod digit_memory_editor;
mod helper;

use prodbg_api::{View, Ui, Service, Reader, Writer, PluginHandler, CViewCallbacks, PDVec2, InputTextFlags, ImGuiStyleVar, EventType};
use std::str;
use number_view::{NumberView, NumberRepresentation, NumberSize, Endianness};
use digit_memory_editor::DigitMemoryEditor;
use helper::get_text_cursor_index;

const START_ADDRESS: usize = 0xf0000;

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

struct MemoryView {
    data: Vec<u8>,
    bytes_requested: usize,
    start_address: InputText,
    bytes_per_line: usize,
    chars_per_address: usize,
    memory_editor: DigitMemoryEditor,
    memory_request: Option<(usize, usize)>,
    number_view: NumberView
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

    fn render_ansi_string(ui: &mut Ui, data: &[u8], char_count: usize) {
        let mut bytes = data.iter();
        let mut text = String::with_capacity(char_count);
        for _ in 0..char_count {
            text.push(match bytes.next() {
                Some(byte) if *byte >= 32 && *byte <= 127 => unsafe { std::char::from_u32_unchecked(*byte as u32) },
                Some(_) => '.',
                None => '?',
            });
        }
        ui.same_line(0, -1);
        ui.text(&text);
    }

    fn set_memory(writer: &mut Writer, address: usize, data: &[u8]) {
        writer.event_begin(EventType::UpdateMemory as u16);
        writer.write_u64("address", address as u64);
        writer.write_data("data", data);
        writer.event_end();
    }

    fn render_line(editor: &mut DigitMemoryEditor, ui: &mut Ui, address: usize, data: &mut [u8], view: NumberView, writer: &mut Writer, columns: usize) -> Option<(usize, usize)> {
        //TODO: Hide editor when user clicks somewhere else
        MemoryView::render_address(ui, address);
        ui.same_line(0, -1);
        let bytes_per_unit = view.size.byte_count();
        let mut cur_address = address;
        let mut next_position = None;
        {
            let mut data_chunks = data.chunks_mut(bytes_per_unit);
            for _ in 0..columns {
                ui.same_line(0, -1);
                match data_chunks.next() {
                    Some(ref mut unit) if unit.len() == bytes_per_unit => {
                        if editor.is_at_address(cur_address) {
                            let (np, data_has_changed) = editor.render(ui, *unit);
                            next_position = np;
                            if data_has_changed {
                                MemoryView::set_memory(writer, cur_address, *unit);
                            }
                        } else {
                            if let Some(index) = MemoryView::render_number(ui, &view.format(*unit)) {
                                if view.representation == NumberRepresentation::Hex {
                                    editor.set_position(cur_address, index);
                                    editor.focus();
                                }
                            }
                        }
                    },
                    _ => {
                        MemoryView::render_inaccessible_memory(ui, view.maximum_chars_needed());
                    }
                }
                cur_address += bytes_per_unit as usize;
            }
        }
        MemoryView::render_ansi_string(ui, data, columns * bytes_per_unit);
        return next_position;
    }

    fn change_number_view(&mut self, view: NumberView) {
        self.number_view = view;
        match view.representation {
            NumberRepresentation::Hex => self.memory_editor.set_number_view(view),
            _ => self.memory_editor.position = None,
        }
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
                // TODO: set limits here. Do not copy more bytes than were reqeusted.
                self.data.resize(data.len(), 0);
                (&mut self.data).copy_from_slice(data);
            },
            Err(err) => {
                println!("Could not read memory: {:?}", err);
            }
        }
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
            start_address: InputText::new(START_ADDRESS),
            bytes_requested: 0,
            bytes_per_line: 8,
            chars_per_address: 10,
            memory_editor: DigitMemoryEditor::new(view),
            memory_request: None,
            number_view: view,
        }
    }

    fn update(&mut self, ui: &mut Ui, reader: &mut Reader, writer: &mut Writer) {
        self.process_events(reader);
        self.render_header(ui);
        let mut address = self.start_address.value;
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

        let line_height = ui.get_text_line_height_with_spacing();
        let (start, end) = ui.calc_list_clipping(line_height);
        let lines_needed = end - start;
        let bytes_needed = bytes_per_line * lines_needed;
        if bytes_needed > self.bytes_requested {
            self.memory_request = Some((self.start_address.get_value(), bytes_needed));
        }

        let mut next_editor_position = None;
        let mut lines = self.data.chunks_mut(bytes_per_line);
        for _ in 0..lines_needed {
            let line = lines.next();
            let buffer = match line {
                Some(data) => data,
                None => &mut [],
            };
            let next_position = MemoryView::render_line(&mut self.memory_editor, ui, address, buffer, self.number_view, writer, bytes_per_line / self.number_view.size.byte_count());
            if next_position.is_some() {
                next_editor_position = next_position;
            }
            address += bytes_per_line;
        }
        if let Some((address, cursor)) = next_editor_position {
            self.memory_editor.set_position(address, cursor);
            self.memory_editor.focus();
        }

        if let Some((address, size)) = self.memory_request {
            println!("Requesting memory {} {}", address, size);
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
