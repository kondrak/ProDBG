//! Memory editor that only allows changing digits and not deleting them.
//! This editor can only be used with Hex number representation as it relies on several properties
//! of it.

use std;
use prodbg_api::{Ui, PDVec2, InputTextFlags, ImGuiStyleVar, InputTextCallbackData, Key};
use number_view::NumberView;
use helper::get_text_cursor_index;

pub struct DigitMemoryEditor {
    /// Address in memory and cursor position
    pub position: Option<(usize, usize)>,
    view: NumberView,
    should_take_focus: bool, // Needed since we cannot change focus in current frame
    should_set_pos_to_start: bool, // Needed since we cannot change cursor position in next frame
}

impl DigitMemoryEditor {
    pub fn new(view: NumberView) -> DigitMemoryEditor {
        DigitMemoryEditor {
            position: None,
            view: view,
            should_take_focus: false,
            should_set_pos_to_start: false,
        }
    }

    pub fn set_position(&mut self, address: usize, cursor: usize) {
        self.position = Some((address, cursor));
    }

    pub fn is_at_address(&self, address: usize) -> bool {
        match self.position {
            Some((x, _)) if x == address => true,
            _ => false,
        }
    }

    pub fn focus(&mut self) {
        self.should_take_focus = true;
        self.should_set_pos_to_start = true;
    }

    pub fn set_number_view(&mut self, view: NumberView) {
        self.view = view;
        self.position = None;
    }

    /// Returns position preceding current. Returns `None` if we're at (0, 0) or `self.position` is
    /// `None`.
    fn previous_position(&self) -> Option<(usize, usize)> {
        self.position.and_then(|(address, cursor)| {
            if cursor == 0 {
                address.checked_sub(self.view.size.byte_count())
                    .map(|address| (address, self.view.maximum_chars_needed() - 1))
            } else {
                Some((address, cursor - 1))
            }
        })
    }

    /// Returns position succeeding current. Returns `None` if address overflows.
    fn next_position(&mut self) -> Option<(usize, usize)> {
        self.position.and_then(|(address, cursor)| {
            if cursor == self.view.maximum_chars_needed() - 1 {
                address.checked_add(self.view.size.byte_count())
                    .map(|address| (address, 0))
            } else {
                Some((address, cursor + 1))
            }
        })
    }

    pub fn render(&mut self, ui: &mut Ui, data: &mut[u8]) -> (Option<(usize, usize)>, bool) {
        let address;
        let cursor;
        if let Some((a, c)) = self.position {
            address = a;
            cursor = c;
        } else {
            return (None, false);
        }
        let text = self.view.format(data);
        let digit_count = text.len();
        let mut next_position = None;
        let mut buf = [text.as_str().as_bytes()[cursor], 0];
        ui.push_style_var_vec(ImGuiStyleVar::ItemSpacing, PDVec2{x: 0.0, y: 0.0});
        if cursor > 0 {
            let left = &text[0..cursor];
            ui.text(left);
            ui.same_line(0, -1);
            if ui.is_item_hovered() && ui.is_mouse_clicked(0, false) {
                next_position = Some((address, get_text_cursor_index(ui, left.len())));
            }
        }

        let mut new_digit = None;
        let width = ui.calc_text_size("f", 0).0;
        if self.should_take_focus {
            ui.set_keyboard_focus_here(0);
            self.should_take_focus = false;
        }
        let flags = InputTextFlags::CharsHexadecimal as i32|InputTextFlags::NoHorizontalScroll as i32|InputTextFlags::AutoSelectAll as i32|InputTextFlags::AlwaysInsertMode as i32|InputTextFlags::CallbackAlways as i32;
        let mut should_set_pos_to_start = self.should_set_pos_to_start;
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
            ui.push_item_width(width);
            ui.push_style_var_vec(ImGuiStyleVar::FramePadding, PDVec2{x: 0.0, y: 0.0});
            // ids are needed to prevent ImGui from reusing old buffer
            ui.push_id_usize(address);
            ui.push_id_usize(cursor);
            ui.input_text("##data", &mut buf, flags, Some(&callback));
            ui.pop_id();
            ui.pop_id();
            ui.pop_style_var(1);
            ui.pop_item_width();
        }
        self.should_set_pos_to_start = should_set_pos_to_start;
        if cursor_pos > 0 {
            // TODO: get rid of unwrap
            let text = std::str::from_utf8(&buf[0..1]).unwrap();
            new_digit = Some(u8::from_str_radix(text, 16).unwrap());
            next_position = self.next_position();
        }

        if let Some(value) = new_digit {
            let offset = (digit_count - cursor - 1) / 2;
            data[offset] = if cursor % 2 == 1 {
                data[offset] & 0b11110000 | value
            } else {
                data[offset] & 0b00001111 | (value << 4)
            };
        }

        if cursor < digit_count {
            ui.same_line(0, -1);
            let right = &text[cursor + 1..digit_count];
            ui.text(right);
            if ui.is_item_hovered() && ui.is_mouse_clicked(0, false) {
                next_position = Some((address, cursor + 1 + get_text_cursor_index(ui, right.len())));
            }
        }

        ui.pop_style_var(1);

        if ui.is_key_pressed(Key::Left, true) {
            next_position = self.previous_position();
        }

        return (next_position, new_digit.is_some());
    }
}
