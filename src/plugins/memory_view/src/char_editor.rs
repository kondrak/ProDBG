//! Memory editor that only allows changing digits and not deleting them.
//! This editor can only be used with Hex number representation as it relies on several properties
//! of it.

use std;
use prodbg_api::{Ui, PDVec2, InputTextFlags, ImGuiStyleVar, InputTextCallbackData, Key};
use helper::get_text_cursor_index;

pub struct CharEditor {
    should_take_focus: bool, // Needed since we cannot change focus in current frame
    should_set_pos_to_start: bool, // Needed since we cannot change cursor position in next frame
}

pub enum NextPosition {
    Left,
    Right,
    Unchanged,
    Changed(usize),
}

impl CharEditor {
    pub fn new() -> CharEditor {
        CharEditor {
            should_take_focus: true,
            should_set_pos_to_start: true,
        }
    }

    pub fn render(&mut self, ui: &mut Ui, text: &str, mut cursor: usize, flags: i32, char_filter: Option<&Fn(char) -> char>) -> (NextPosition, Option<String>) {
        if text.len() == 0 {
            return (NextPosition::Unchanged, None);
        }
        let digit_count = text.len();
        let mut next_position = NextPosition::Unchanged;
        if cursor >= text.len() {
            cursor = text.len() - 1;
        }
        let mut buf = [text.as_bytes()[cursor], 0];
        ui.push_style_var_vec(ImGuiStyleVar::ItemSpacing, PDVec2{x: 0.0, y: 0.0});
        if cursor > 0 {
            let left = &text[0..cursor];
            ui.text(left);
            ui.same_line(0, -1);
            if ui.is_item_hovered() && ui.is_mouse_clicked(0, false) {
                next_position = NextPosition::Changed(get_text_cursor_index(ui, left.len()));
            }
        }

        let width = ui.calc_text_size("f", 0).0;
        if self.should_take_focus {
            ui.set_keyboard_focus_here(0);
            self.should_take_focus = false;
        }
        let flags = flags|InputTextFlags::NoHorizontalScroll as i32|InputTextFlags::AutoSelectAll as i32|InputTextFlags::AlwaysInsertMode as i32|InputTextFlags::CallbackAlways as i32|InputTextFlags::CallbackCharFilter as i32;
        let mut should_set_pos_to_start = self.should_set_pos_to_start;
        let mut cursor_pos = 0;
        let mut text_has_changed = false;
        {
            let callback = |mut data: InputTextCallbackData| {
                let flag = data.get_event_flag();
                if flag == InputTextFlags::CallbackAlways as i32 {
                    if should_set_pos_to_start {
                        data.set_cursor_pos(0);
                        should_set_pos_to_start = false;
                    } else {
                        cursor_pos = data.get_cursor_pos();
                    }
                }
                if flag == InputTextFlags::CallbackCharFilter as i32 {
                    if let Some(c) = data.get_event_char() {
                        if let Some(filter) = char_filter {
                            let filtered_char = filter(c);
                            data.set_event_char(filtered_char);
                            text_has_changed = filtered_char != '\u{0}';
                        } else {
                            text_has_changed = c != '\u{0}';
                        }
                    }
                }
            };
            ui.push_item_width(width);
            ui.push_style_var_vec(ImGuiStyleVar::FramePadding, PDVec2{x: 0.0, y: 0.0});
            ui.input_text("##data", &mut buf, flags, Some(&callback));
            ui.pop_style_var(1);
            ui.pop_item_width();
        }
        self.should_set_pos_to_start = should_set_pos_to_start;
        let mut changed_text = None;
        if cursor_pos > 0 {
            next_position = if cursor == digit_count - 1 {
                NextPosition::Right
            } else {
                NextPosition::Changed(cursor + 1)
            }
        }
        if text_has_changed {
            changed_text = std::str::from_utf8(&buf[0..1]).ok().map(|s| s.to_owned());
        }

        if cursor < digit_count {
            ui.same_line(0, -1);
            let right = &text[cursor + 1..digit_count];
            ui.text(right);
            if ui.is_item_hovered() && ui.is_mouse_clicked(0, false) {
                next_position = NextPosition::Changed(cursor + 1 + get_text_cursor_index(ui, right.len()));
            }
        }

        ui.pop_style_var(1);

        if ui.is_key_pressed(Key::Left, true) {
            next_position = if cursor > 0 {
                NextPosition::Changed(cursor - 1)
            } else {
                NextPosition::Left
            }
        }

        return (next_position, changed_text);
    }
}
