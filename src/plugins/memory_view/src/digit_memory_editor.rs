//! Memory editor that only allows changing digits and not deleting them.
//! This editor can only be used with Hex number representation as it relies on several properties
//! of it.

use std;
use prodbg_api::{Ui, PDVec2, InputTextFlags, ImGuiStyleVar, InputTextCallbackData};
use number_view::NumberView;
use helper::get_text_cursor_index;

pub struct DigitMemoryEditor {
    // TODO: move cursor into Option
    pub address: Option<usize>, // Address for first digit
    cursor: usize, // Position of digit edited (0 = leftmost digit)
    view: NumberView,
    should_take_focus: bool, // Needed since we cannot change focus in current frame
    should_set_pos_to_start: bool, // Needed since we cannot change cursor position in next frame
}

impl DigitMemoryEditor {
    pub fn new(view: NumberView) -> DigitMemoryEditor {
        DigitMemoryEditor {
            view: view,
            address: None,
            cursor: 0,
            should_take_focus: false,
            should_set_pos_to_start: false,
        }
    }

    pub fn set_address(&mut self, address: usize, cursor: usize) {
        self.address = Some(address);
        self.cursor = cursor;
    }

    pub fn focus(&mut self) {
        self.should_take_focus = true;
        self.should_set_pos_to_start = true;
    }

    pub fn set_number_view(&mut self, view: NumberView) {
        self.view = view;
    }

    pub fn set_inactive(&mut self) {
        self.address = None;
    }

    pub fn render(&mut self, ui: &mut Ui, data: &mut[u8]) -> bool {
        let text = self.view.format(data);
        let digit_count = text.len();
        let mut next_cursor = None;
        let mut buf = [text.as_str().as_bytes()[self.cursor], 0];
        ui.push_style_var_vec(ImGuiStyleVar::ItemSpacing, PDVec2{x: 0.0, y: 0.0});
        if self.cursor > 0 {
            let left = &text[0..self.cursor];
            ui.text(left);
            ui.same_line(0, -1);
            if ui.is_item_hovered() && ui.is_mouse_clicked(0, false) {
                next_cursor = Some(get_text_cursor_index(ui, left.len()));
            }
        }

        let mut new_digit = None;
        let width = ui.calc_text_size("f", 0).0;
        if self.should_take_focus {
            ui.set_keyboard_focus_here(0);
            self.should_take_focus = false;
        }
        let flags = InputTextFlags::CharsHexadecimal as i32|InputTextFlags::NoHorizontalScroll as i32|InputTextFlags::AlwaysInsertMode as i32|InputTextFlags::CallbackAlways as i32;
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
            ui.input_text("##data", &mut buf, flags, Some(&callback));
            ui.pop_style_var(1);
            ui.pop_item_width();
        }
        if cursor_pos > 0 {
            // TODO: get rid of unwrap
            let text = std::str::from_utf8(&buf[0..1]).unwrap();
            new_digit = Some(u8::from_str_radix(text, 16).unwrap());
            self.set_inactive();
        }
        self.should_set_pos_to_start = should_set_pos_to_start;

        if let Some(value) = new_digit {
            let offset = (digit_count - self.cursor) / 2;
            data[offset] = if self.cursor % 2 == 0 {
                data[offset] & 0b00001111 | (value << 4)
            } else {
                data[offset] & 0b11110000 | value
            };
        }

        if self.cursor < digit_count {
            ui.same_line(0, -1);
            let right = &text[self.cursor + 1..digit_count];
            ui.text(right);
            if ui.is_item_hovered() && ui.is_mouse_clicked(0, false) {
                next_cursor = Some(self.cursor + 1 + get_text_cursor_index(ui, right.len()));
            }
        }

        ui.pop_style_var(1);

        if let Some(cursor) = next_cursor {
            self.cursor = cursor;
        }
        return new_digit.is_some();
    }
}
