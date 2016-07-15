//! Structures describing number view:
//! * representation (hex, signed decimal, unsigned decimal, float)
//! * size (one to eight bytes)
//! Also capable of formatting memory (slice of u8) into such view.


use std;

#[derive(Debug, Clone, Copy)]
pub struct NumberView {
    pub representation: NumberRepresentation,
    pub size: NumberSize,
    pub endianness: Endianness
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
            NumberRepresentation::Float => match self.size {
                NumberSize::FourBytes => 14,
                NumberSize::EightBytes => 23,
                _ => 5, // For "Error" message
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
                let mut buf_copy = [0; $len];
                buf_copy.copy_from_slice(&buffer[0..$len]);
                unsafe {
                    // Cannot use transmute_copy here as it requires argument with constant size
                    // known at compile-time
                    let mut num: $data_type = std::mem::transmute(buf_copy);
                    num = match $endianness {
                        Endianness::Little => num.to_le(),
                        Endianness::Big => num.to_be(),
                    };
                    return format!($format, num);
                }
            };
            ($data_type:ty, $len:expr, $format:expr) => {
                let mut buf_copy = [0; $len];
                buf_copy.copy_from_slice(&buffer[0..$len]);
                unsafe {
                    // Cannot use transmute_copy here as it requires argument with constant size
                    // known at compile-time
                    let num: $data_type = std::mem::transmute(buf_copy);
                    return format!($format, num);
                }
            };
        }
        match self.representation {
            NumberRepresentation::Hex => match self.size {
                NumberSize::OneByte => {format_buffer!(u8, 1, self.endianness, "{:02x}");}
                NumberSize::TwoBytes => {format_buffer!(u16, 2, self.endianness, "{:04x}");}
                NumberSize::FourBytes => {format_buffer!(u32, 4, self.endianness, "{:08x}");}
                NumberSize::EightBytes => {format_buffer!(u64, 8, self.endianness, "{:016x}");}
            },
            NumberRepresentation::UnsignedDecimal => match self.size {
                NumberSize::OneByte => {format_buffer!(u8, 1, self.endianness, "{:3}");}
                NumberSize::TwoBytes => {format_buffer!(u16, 2, self.endianness, "{:5}");}
                NumberSize::FourBytes => {format_buffer!(u32, 4, self.endianness, "{:10}");}
                NumberSize::EightBytes => {format_buffer!(u64, 8, self.endianness, "{:20}");}
            },
            NumberRepresentation::SignedDecimal => match self.size {
                NumberSize::OneByte => {format_buffer!(i8, 1, self.endianness, "{:4}");}
                NumberSize::TwoBytes => {format_buffer!(i16, 2, self.endianness, "{:6}");}
                NumberSize::FourBytes => {format_buffer!(i32, 4, self.endianness, "{:11}");}
                NumberSize::EightBytes => {format_buffer!(i64, 8, self.endianness, "{:20}");}
            },
            NumberRepresentation::Float => match self.size {
                NumberSize::FourBytes => {format_buffer!(f32, 4, "{:14e}");}
                NumberSize::EightBytes => {format_buffer!(f64, 8, "{:23e}");}
                // Should never be available to pick through user interface
                _ => return "Error".to_owned()
            },
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

static NUMBER_REPRESENTATION_NAMES: [&'static str; 4] = ["Hex", "Unsigned decimal", "Signed decimal", "Float"];
static FLOAT_AVAILABLE_SIZES: [NumberSize; 2] = [NumberSize::FourBytes, NumberSize::EightBytes];
static OTHER_AVAILABLE_SIZES: [NumberSize; 4] = [NumberSize::OneByte, NumberSize::TwoBytes, NumberSize::FourBytes, NumberSize::EightBytes];
impl NumberRepresentation {
    // TODO: make this example work as test. Could not run it as a test using `cargo test`
    /// Converts this number into index, which matches `NumberRepresentation::names()`
    /// # Examples
    /// ```
    /// use NumberRepresentation;
    /// let names = NumberRepresentation::names();
    /// assert_eq!("Hex", names[NumberRepresentation::Hex.as_usize()]);
    /// ```
    pub fn as_usize(&self) -> usize {
        match *self {
            NumberRepresentation::Hex => 0,
            NumberRepresentation::UnsignedDecimal => 1,
            NumberRepresentation::SignedDecimal => 2,
            NumberRepresentation::Float => 3,
        }
    }

    /// Converts index into `NumberRepresentation`. Uses `NumberRepresentation::Hex` if index does
    /// not match any.
    pub fn from_usize(id: usize) -> NumberRepresentation {
        match id {
            0 => NumberRepresentation::Hex,
            1 => NumberRepresentation::UnsignedDecimal,
            2 => NumberRepresentation::SignedDecimal,
            3 => NumberRepresentation::Float,
            _ => NumberRepresentation::Hex,
        }
    }

    pub fn can_be_of_size(&self, size: NumberSize) -> bool {
        match *self {
            NumberRepresentation::Float => match size {
                NumberSize::FourBytes => true,
                NumberSize::EightBytes => true,
                _ => false,
            },
            _ => true
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

    /// Returns names for all possible representations. Index matches
    /// `NumberRepresentation::as_usize`.
    pub fn names() -> &'static [&'static str] {
        &NUMBER_REPRESENTATION_NAMES
    }
}

static ENDIANNESS_NAMES: [&'static str; 2] = ["Little-endian", "Big-endian"];
impl Endianness {
    // TODO: make this example work as test. Could not run it as a test using `cargo test`
    /// Converts this endianness into index, which matches `Endianness::names()`
    /// # Examples
    /// ```
    /// use Endianness;
    /// let names = Endianness::names();
    /// assert_eq!("Little-endian", names[Endianness::Little.as_usize()]);
    /// ```
    pub fn as_usize(&self) -> usize {
        match *self {
            Endianness::Little => 0,
            Endianness::Big => 1,
        }
    }

    /// Converts index into `Endianness`. Uses default endianness for target build if index does
    /// not match any.
    pub fn from_usize(id: usize) -> Endianness {
        match id {
            0 => Endianness::Little,
            1 => Endianness::Big,
            _ => Endianness::default(),
        }
    }

    /// Returns names for all possible representations. Index matches `Endianness::as_usize`.
    pub fn names() -> &'static [&'static str] {
        &ENDIANNESS_NAMES
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