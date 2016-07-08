use std::ptr;
use ui_ffi::*;
use std::fmt;
use std::fmt::Write;
use scintilla::Scintilla;
use std::os::raw::{c_void};

use CFixedString;

/// Taken from minifb. Not sure if we can share this somehow?
/// Key is used by the get key functions to check if some keys on the keyboard has been pressed
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Key {
    Key0 = 0,
    Key1 = 1,
    Key2 = 2,
    Key3 = 3,
    Key4 = 4,
    Key5 = 5,
    Key6 = 6,
    Key7 = 7,
    Key8 = 8,
    Key9 = 9,

    A = 10,
    B = 11,
    C = 12,
    D = 13,
    E = 14,
    F = 15,
    G = 16,
    H = 17,
    I = 18,
    J = 19,
    K = 20,
    L = 21,
    M = 22,
    N = 23,
    O = 24,
    P = 25,
    Q = 26,
    R = 27,
    S = 28,
    T = 29,
    U = 30,
    V = 31,
    W = 32,
    X = 33,
    Y = 34,
    Z = 35,

    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,

    Down,
    Left,
    Right,
    Up,
    Apostrophe,
    Backquote,

    Backslash,
    Comma,
    Equal,
    LeftBracket,
    Minus,
    Period,
    RightBracket,
    Semicolon,

    Slash,
    Backspace,
    Delete,
    End,
    Enter,

    Escape,

    Home,
    Insert,
    Menu,

    PageDown,
    PageUp,

    Pause,
    Space,
    Tab,
    NumLock,
    CapsLock,
    ScrollLock,
    LeftShift,
    RightShift,
    LeftCtrl,
    RightCtrl,

    NumPad0,
    NumPad1,
    NumPad2,
    NumPad3,
    NumPad4,
    NumPad5,
    NumPad6,
    NumPad7,
    NumPad8,
    NumPad9,
    NumPadDot,
    NumPadSlash,
    NumPadAsterisk,
    NumPadMinus,
    NumPadPlus,
    NumPadEnter,

    LeftAlt,
    RightAlt,

    LeftSuper,
    RightSuper,

    /// Used when an Unknown key has been pressed
    Unknown,

    Count = 107,
}

// Flags for ImGui::InputText()
pub enum InputTextFlags {
    // Default: 0
    CharsDecimal        = 1 << 0,   // Allow 0123456789.+-*/
    CharsHexadecimal    = 1 << 1,   // Allow 0123456789ABCDEFabcdef
    CharsUppercase      = 1 << 2,   // Turn a..z into A..Z
    CharsNoBlank        = 1 << 3,   // Filter out spaces, tabs
    AutoSelectAll       = 1 << 4,   // Select entire text when first taking mouse focus
    EnterReturnsTrue    = 1 << 5,   // Return 'true' when Enter is pressed (as opposed to when the value was modified)
    CallbackCompletion  = 1 << 6,   // Call user function on pressing TAB (for completion handling)
    CallbackHistory     = 1 << 7,   // Call user function on pressing Up/Down arrows (for history handling)
    CallbackAlways      = 1 << 8,   // Call user function every time. User code may query cursor position, modify text buffer.
    CallbackCharFilter  = 1 << 9,   // Call user function to filter character. Modify data->EventChar to replace/filter input, or return 1 to discard character.
    AllowTabInput       = 1 << 10,  // Pressing TAB input a '\t' character into the text field
    CtrlEnterForNewLine = 1 << 11,  // In multi-line mode, allow exiting edition by pressing Enter. Ctrl+Enter to add new line (by default adds new lines with Enter).
    NoHorizontalScroll  = 1 << 12,  // Disable following the cursor horizontally
    AlwaysInsertMode    = 1 << 13,  // Insert mode
    ReadOnly            = 1 << 14,  // Read-only mode
    Password            = 1 << 15,  // Password mode, display all characters as '*'
    // [Internal]
    Multiline           = 1 << 20   // For internal use by InputTextMultiline()
}

pub enum ImGuiStyleVar
{
    Alpha = 0,           // float
    WindowPadding,       // ImVec2
    WindowRounding,      // float
    WindowMinSize,       // ImVec2
    ChildWindowRounding, // float
    FramePadding,        // ImVec2
    FrameRounding,       // float
    ItemSpacing,         // ImVec2
    ItemInnerSpacing,    // ImVec2
    IndentSpacing,       // float
    GrabMinSize          // float
}


#[derive(Clone)]
pub struct Ui {
    pub api: *mut CPdUI,
    fmt_buffer: String,
}

#[derive(Clone, Copy, Debug)]
pub struct Color {
    color: u32,
}

impl Color {
    pub fn from_u32(color: u32) -> Color {
        Color { color: color }
    }

    pub fn from_rgb(r: u32, g: u32, b: u32) -> Color {
        Self::from_u32((255 << 24) | (r << 16) | (g << 8) | b)
    }

    pub fn from_rgba(r: u32, g: u32, b: u32, a: u32) -> Color {
        Self::from_u32((a << 24) | (r << 16) | (g << 8) | b)
    }

    pub fn from_argb(a: u32, r: u32, g: u32, b: u32) -> Color {
        Self::from_u32((a << 24) | (r << 16) | (g << 8) | b)
    }
    pub fn from_au32(a: u32, rgb: u32) -> Color {
        Self::from_u32((a << 24) | rgb)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub fn new(x: f32, y: f32) -> Vec2 {
        Vec2 { x: x, y: y }
    }
}

macro_rules! true_is_1 {
    ($e:expr) => (if $e { 1 } else { 0 })
}

impl Ui {
    pub fn new(native_api: *mut CPdUI) -> Ui {
        Ui {
            api: native_api,
            fmt_buffer: String::with_capacity(2048),
        }
    }

    pub fn set_title(&self, title: &str) {
        unsafe {
            let t = CFixedString::from_str(title).as_ptr();
            ((*self.api).set_title)((*self.api).private_data, t);
        }
    }

    #[inline]
    pub fn get_window_size(&self) -> (f32, f32) {
        unsafe {
            let t = ((*self.api).get_window_size)();
            (t.x, t.y)
        }
    }

    #[inline]
    pub fn get_window_pos(&self) -> PDVec2 {
        unsafe { ((*self.api).get_window_pos)() }
    }

    #[inline]
    pub fn get_text_line_height_with_spacing(&self) -> f32 {
        unsafe { ((*self.api).get_text_line_height_with_spacing)() as f32 }
    }

    #[inline]
    pub fn get_cursor_pos(&self) -> (f32, f32) {
        unsafe {
            let t = ((*self.api).get_cursor_pos)();
            (t.x, t.y)
        }
    }

    #[inline]
    pub fn set_cursor_pos(&self, pos: (f32, f32)) {
        unsafe {
            ((*self.api).set_cursor_pos)(PDVec2{x: pos.0, y: pos.1});
        }
    }

    #[inline]
    pub fn get_cursor_screen_pos(&self) -> (f32, f32) {
        unsafe {
            let t = ((*self.api).get_cursor_screen_pos)();
            (t.x, t.y)
        }
    }

    #[inline]
    pub fn set_cursor_screen_pos(&self, pos: (f32, f32)) {
        unsafe {
            ((*self.api).set_cursor_screen_pos)(PDVec2{x: pos.0, y: pos.1});
        }
    }

    #[inline]
    pub fn fill_rect(&self, x: f32, y: f32, width: f32, height: f32, col: Color) {
        unsafe {
            ((*self.api).fill_rect)(PDRect { x: x, y: y, width: width, height: height }, col.color);
        }
    }

    #[inline]
    pub fn set_scroll_here(&self, center: f32) {
        unsafe { ((*self.api).set_scroll_here)(center) }
    }

    pub fn begin_child(&self, id: &str, pos: Option<PDVec2>, border: bool, flags: u32) {
        unsafe {
            let t = CFixedString::from_str(id).as_ptr();
            match pos {
                Some(p) => ((*self.api).begin_child)(t, p, border as i32, flags as i32),
                None => {
                    ((*self.api).begin_child)(t,
                                              PDVec2 { x: 0.0, y: 0.0 },
                                              border as i32,
                                              flags as i32)
                }
            }
        }
    }

    #[inline]
    pub fn end_child(&self) {
        unsafe { ((*self.api).end_child)() }
    }

    #[inline]
    pub fn get_scroll_y(&self) -> f32 {
        unsafe { ((*self.api).get_scroll_y)() as f32 }
    }

    #[inline]
    pub fn get_scroll_max_y(&self) -> f32 {
        unsafe { ((*self.api).get_scroll_max_y)() as f32 }
    }

    #[inline]
    pub fn set_scroll_y(&self, pos: f32) {
        unsafe { ((*self.api).set_scroll_y)(pos) }
    }

    #[inline]
    pub fn set_scroll_from_pos_y(&self, pos_y: f32, center_ratio: f32) {
        unsafe { ((*self.api).set_scroll_from_pos_y)(pos_y, center_ratio) }
    }

    #[inline]
    pub fn set_keyboard_focus_here(&self, offset: i32) {
        unsafe { ((*self.api).set_keyboard_focus_here)(offset) }
    }

    // TODO: push/pop font

    #[inline]
	pub fn push_style_color(&self, index: usize, col: Color) {
        unsafe { ((*self.api).push_style_color)(index as u32, col.color) }
    }

    #[inline]
	pub fn pop_style_color(&self, index: usize) {
        unsafe { ((*self.api).pop_style_color)(index as i32) }
    }

    #[inline]
	pub fn push_style_var(&self, index: ImGuiStyleVar, val: f32) {
        unsafe { ((*self.api).push_style_var)(index as u32, val) }
    }

    #[inline]
	pub fn push_style_var_vec(&self, index: ImGuiStyleVar, val: PDVec2) {
        unsafe { ((*self.api).push_style_var_vec)(index as u32, val) }
    }

    #[inline]
	pub fn pop_style_var(&self, count: usize) {
        unsafe { ((*self.api).pop_style_var)(count as i32) }
    }

    #[inline]
    pub fn get_font_size(&self) -> f32 {
        unsafe { ((*self.api).get_font_size)() }
    }

    #[inline]
    pub fn push_item_width(&self, width: f32) {
        unsafe { ((*self.api).push_item_width)(width) }
    }

    #[inline]
    pub fn pop_item_width(&self) {
        unsafe { ((*self.api).pop_item_width)(); }
    }

    #[inline]
    pub fn same_line(&self, column_x: i32, spacing_w: i32) {
        unsafe { ((*self.api).same_line)(column_x, spacing_w) }
    }

    #[inline]
    pub fn is_item_hovered(&self) -> bool {
        unsafe { ((*self.api).is_item_hovered)() != 0}
    }

    #[inline]
    pub fn is_mouse_clicked(&self, button: i32, repeat: bool) -> bool {
        unsafe { ((*self.api).is_mouse_clicked)(button, true_is_1!(repeat)) != 0}
    }

    #[inline]
    pub fn checkbox(&self, label: &str, state: &mut bool) -> bool{
        unsafe {
            let c_label = CFixedString::from_str(label).as_ptr();
            let mut c_state: i32 = if *state { 1 } else { 0 };
            let res = ((*self.api).checkbox)(c_label, &mut c_state) != 0;
            *state = c_state != 0;
            return res;
        };
    }

    #[inline]
    pub fn input_text(&self, label: &str, buf: &mut [u8], flags: i32) -> bool {
        unsafe {
            let c_label = CFixedString::from_str(label).as_ptr();
            let buf_len = buf.len() as i32;
            let buf_pointer = buf.as_mut_ptr() as *mut i8;
            extern fn null_callback(_: *mut PDUIInputTextCallbackData) {}
            ((*self.api).input_text)(c_label, buf_pointer, buf_len, flags, null_callback, ptr::null_mut()) != 0
        }
    }

    #[inline]
    pub fn calc_text_size(&self, text: &str, offset: usize) -> (f32, f32) {
        unsafe {
            let start = CFixedString::from_str(text);
            if offset == 0 {
                let t = ((*self.api).calc_text_size)(start.as_ptr(), ptr::null(), 0, -1.0);
                (t.x, t.y)
            } else {
                let slice = &start.as_str()[offset..];
                let t = ((*self.api).calc_text_size)(start.as_ptr(), slice.as_ptr(), 0, -1.0);
                (t.x, t.y)
            }
        }
    }

    // Text

    pub fn text(&self, text: &str) {
        unsafe {
            let t = CFixedString::from_str(text).as_ptr();
            ((*self.api).text)(t);
        }
    }

    pub fn text_fmt(&mut self, args: fmt::Arguments) {
        unsafe {
            self.fmt_buffer.clear();
            write!(&mut self.fmt_buffer, "{}", args).unwrap();
            let t = CFixedString::from_str(&self.fmt_buffer).as_ptr();
            ((*self.api).text)(t);
        }
    }

    pub fn text_colored(&self, col: Color, text: &str) {
        unsafe {
            let t = CFixedString::from_str(text).as_ptr();
            ((*self.api).text_colored)(col.color, t);
        }
    }

    pub fn text_disabled(&self, text: &str) {
        unsafe {
            let t = CFixedString::from_str(text).as_ptr();
            ((*self.api).text_disabled)(t);
        }
    }

    pub fn text_wrapped(&self, text: &str) {
        unsafe {
            let t = CFixedString::from_str(text).as_ptr();
            ((*self.api).text_wrapped)(t);
        }
    }

	pub fn columns(&self, count: isize, id: Option<&str>, border: bool) {
	    unsafe {
            match id {
                Some(p) => {
                    let t = CFixedString::from_str(p).as_ptr();
                    ((*self.api).columns)(count as i32, t, border as i32)
                }
                None => ((*self.api).columns)(count as i32, ptr::null(), border as i32),
            }
        }
    }

    #[inline]
    pub fn next_column(&self) {
        unsafe { ((*self.api).next_column)() }
    }

    pub fn button(&self, title: &str, pos: Option<PDVec2>) -> bool {
        unsafe {
            let t = CFixedString::from_str(title).as_ptr();
            match pos {
                Some(p) => ((*self.api).button)(t, p) != 0,
                None => ((*self.api).button)(t, PDVec2 { x: 0.0, y: 0.0 }) != 0,
            }
        }
    }
    pub fn begin_popup(&self, text: &str) -> bool {
        unsafe {
            let t = CFixedString::from_str(text).as_ptr();
            ((*self.api).begin_popup)(t) == 1
        }
    }

    pub fn begin_menu(&self, text: &str, enabled: bool) -> bool {
        unsafe {
            let t = CFixedString::from_str(text).as_ptr();
            let s = if enabled { 1 } else { 0 };
            ((*self.api).begin_menu)(t, s) == 1
        }
    }

    pub fn open_popup(&self, text: &str) {
        unsafe {
            let t = CFixedString::from_str(text).as_ptr();
            ((*self.api).open_popup)(t);
        }
    }

	pub fn menu_item(&self, text: &str, selected: bool, enabled: bool) -> bool {
        unsafe {
            let name = CFixedString::from_str(text).as_ptr();
            let s = if selected { 1 } else { 0 };
            let e = if enabled { 1 } else { 0 };
            ((*self.api).menu_item)(name, ptr::null(), s, e) == 1
        }
    }

    pub fn end_menu(&self) {
        unsafe { ((*self.api).end_menu)() }
    }

    pub fn end_popup(&self) {
        unsafe { ((*self.api).end_popup)() }
    }

	pub fn sc_input_text(&self, title: &str, width: usize, height: usize) -> Scintilla {
	    unsafe {
            let name = CFixedString::from_str(title);
	        Scintilla::new(((*self.api).sc_input_text)(name.as_ptr(),
	                        width as f32,
	                        height as f32, None))
        }
    }


    ///
    /// Keyboard support
    ///

	pub fn is_key_down(&self, key: Key) -> bool {
        unsafe { ((*self.api).is_key_down)(key as i32) == 1 }
    }

	pub fn is_key_pressed(&self, key: Key, repeat: bool) -> bool {
        unsafe { ((*self.api).is_key_pressed)(key as i32, true_is_1!(repeat)) == 1 }
    }

	pub fn is_key_released(&self, key: Key) -> bool {
        unsafe { ((*self.api).is_key_released)(key as i32) == 1 }
    }

    ///
    /// Rendering primitives
    ///

    pub fn fill_convex_poly(&self, vertices: &[Vec2], col: Color, anti_aliased: bool) {
	    unsafe {
            ((*self.api).fill_convex_poly)(
                vertices.as_ptr() as *const c_void,
                vertices.len() as u32,
                col.color,
                true_is_1!(anti_aliased))
        }
    }

    pub fn fill_circle(&self, pos: &Vec2, radius: f32, col: Color, segment_count: usize, anti_aliased: bool) {
	    unsafe {
            ((*self.api).fill_circle)(
                PDVec2 { x: pos.x, y: pos.y },
                radius,
                col.color,
                segment_count as u32,
                true_is_1!(anti_aliased))
        }
    }
}

