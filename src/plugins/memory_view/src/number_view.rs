//! Structures describing number view:
//! * representation (hex, signed decimal, unsigned decimal, float)
//! * size (one to eight bytes)
//! Also capable of formatting memory (slice of u8) into such view.


use std;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NumberView {
    pub representation: NumberRepresentation,
    pub size: NumberSize,
    pub endianness: Endianness,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NumberRepresentation {
    Hex,
    UnsignedDecimal,
    SignedDecimal,
    Float,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NumberSize {
    OneByte,
    TwoBytes,
    FourBytes,
    EightBytes,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Endianness {
    Little,
    Big,
}

impl NumberView {
    /// Maximum number of characters needed to show number
    // TODO: change to calculation from MAX/MIN when `const fn` is in stable Rust
    pub fn maximum_chars_needed(&self) -> usize {
        match self.representation {
            NumberRepresentation::Hex => self.size.byte_count() * 2,
            NumberRepresentation::UnsignedDecimal => {
                match self.size {
                    NumberSize::OneByte => 3,
                    NumberSize::TwoBytes => 5,
                    NumberSize::FourBytes => 10,
                    NumberSize::EightBytes => 20,
                }
            }
            NumberRepresentation::SignedDecimal => {
                match self.size {
                    NumberSize::TwoBytes => 6,
                    NumberSize::OneByte => 4,
                    NumberSize::FourBytes => 11,
                    NumberSize::EightBytes => 20,
                }
            }
            NumberRepresentation::Float => {
                match self.size {
                    NumberSize::FourBytes => 14,
                    NumberSize::EightBytes => 23,
                    _ => 5, // For "Error" message
                }
            }
        }
    }

    /// Format memory. Returns "Error" if representation and size do not match (one- and two-bytes
    /// float currently).
    /// # Panics
    /// Panics if slice of memory is less than number size.
    pub fn format(&self, buffer: &[u8]) -> String {
        macro_rules! format_buffer {
            ($data_type:ty, $len:expr, $endianness:expr, $format:expr) => {
                unsafe {
                    if buffer.len() < $len {
                        panic!("Could not convert buffer of length {} into data type of size {}", buffer.len(), $len);
                    }
                    let num_ref: &$data_type = std::mem::transmute(buffer.as_ptr());
                    let num = match $endianness {
                        Endianness::Little => num_ref.to_le(),
                        Endianness::Big => num_ref.to_be(),
                    };
                    return format!($format, num);
                }
            };
            ($data_type:ty, $len:expr, $format:expr) => {
                unsafe {
                    if buffer.len() < $len {
                        panic!("Could not convert buffer of length {} into data type of size {}", buffer.len(), $len);
                    }
                    let num_ref: &$data_type = std::mem::transmute(buffer.as_ptr());
                    return format!($format, num_ref);
                }
            };
        }
        match self.representation {
            NumberRepresentation::Hex => {
                match self.size {
                    NumberSize::OneByte => format_buffer!(u8, 1, self.endianness, "{:02x}"),
                    NumberSize::TwoBytes => format_buffer!(u16, 2, self.endianness, "{:04x}"),
                    NumberSize::FourBytes => format_buffer!(u32, 4, self.endianness, "{:08x}"),
                    NumberSize::EightBytes => format_buffer!(u64, 8, self.endianness, "{:016x}"),
                }
            }
            NumberRepresentation::UnsignedDecimal => {
                match self.size {
                    NumberSize::OneByte => format_buffer!(u8, 1, self.endianness, "{:3}"),
                    NumberSize::TwoBytes => format_buffer!(u16, 2, self.endianness, "{:5}"),
                    NumberSize::FourBytes => format_buffer!(u32, 4, self.endianness, "{:10}"),
                    NumberSize::EightBytes => format_buffer!(u64, 8, self.endianness, "{:20}"),
                }
            }
            NumberRepresentation::SignedDecimal => {
                match self.size {
                    NumberSize::OneByte => format_buffer!(i8, 1, self.endianness, "{:4}"),
                    NumberSize::TwoBytes => format_buffer!(i16, 2, self.endianness, "{:6}"),
                    NumberSize::FourBytes => format_buffer!(i32, 4, self.endianness, "{:11}"),
                    NumberSize::EightBytes => format_buffer!(i64, 8, self.endianness, "{:20}"),
                }
            }
            NumberRepresentation::Float => {
                match self.size {
                    NumberSize::FourBytes => format_buffer!(f32, 4, "{:14e}"),
                    NumberSize::EightBytes => format_buffer!(f64, 8, "{:23e}"),
                    // Should never be available to pick through user interface
                    _ => return "Error".to_owned(),
                }
            }
        }
    }

    /// Changes number representation and picks default size if current size do not match new
    /// representation.
    pub fn change_representation(&mut self, representation: NumberRepresentation) {
        self.representation = representation;
        if !representation.can_be_of_size(self.size) {
            self.size = representation.get_default_size();
        }
    }
}

impl Default for NumberView {
    fn default() -> NumberView {
        NumberView {
            representation: NumberRepresentation::Hex,
            size: NumberSize::OneByte,
            endianness: Endianness::default(),
        }
    }
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

static FLOAT_AVAILABLE_SIZES: [NumberSize; 2] = [NumberSize::FourBytes, NumberSize::EightBytes];
static OTHER_AVAILABLE_SIZES: [NumberSize; 4] =
    [NumberSize::OneByte, NumberSize::TwoBytes, NumberSize::FourBytes, NumberSize::EightBytes];
impl NumberRepresentation {
    pub fn can_be_of_size(&self, size: NumberSize) -> bool {
        match *self {
            NumberRepresentation::Float => {
                match size {
                    NumberSize::FourBytes => true,
                    NumberSize::EightBytes => true,
                    _ => false,
                }
            }
            _ => true,
        }
    }

    pub fn get_avaialable_sizes(&self) -> &'static [NumberSize] {
        match *self {
            NumberRepresentation::Float => &FLOAT_AVAILABLE_SIZES,
            _ => &OTHER_AVAILABLE_SIZES,
        }
    }

    pub fn get_default_size(&self) -> NumberSize {
        match *self {
            NumberRepresentation::Float => NumberSize::FourBytes,
            _ => NumberSize::OneByte,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match *self {
            NumberRepresentation::Hex => "Hex",
            NumberRepresentation::UnsignedDecimal => "Unsigned decimal",
            NumberRepresentation::SignedDecimal => "Signed decimal",
            NumberRepresentation::Float => "Float",
        }
    }
}

impl Endianness {
    pub fn as_str(&self) -> &'static str {
        match self {
            &Endianness::Little => "Little-endian",
            &Endianness::Big => "Big-endian",
        }
    }
}

impl Default for Endianness {
    fn default() -> Endianness {
        if cfg!(target_endian = "little") {
            Endianness::Little
        } else {
            Endianness::Big
        }
    }
}
