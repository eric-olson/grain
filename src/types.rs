use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectType {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
    F32,
    F64,
}

impl InspectType {
    pub const ALL: [InspectType; 10] = [
        InspectType::U8,
        InspectType::I8,
        InspectType::U16,
        InspectType::I16,
        InspectType::U32,
        InspectType::I32,
        InspectType::U64,
        InspectType::I64,
        InspectType::F32,
        InspectType::F64,
    ];

    pub fn byte_size(self) -> usize {
        match self {
            InspectType::U8 | InspectType::I8 => 1,
            InspectType::U16 | InspectType::I16 => 2,
            InspectType::U32 | InspectType::I32 | InspectType::F32 => 4,
            InspectType::U64 | InspectType::I64 | InspectType::F64 => 8,
        }
    }
}

impl fmt::Display for InspectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InspectType::U8 => write!(f, "u8"),
            InspectType::I8 => write!(f, "i8"),
            InspectType::U16 => write!(f, "u16"),
            InspectType::I16 => write!(f, "i16"),
            InspectType::U32 => write!(f, "u32"),
            InspectType::I32 => write!(f, "i32"),
            InspectType::U64 => write!(f, "u64"),
            InspectType::I64 => write!(f, "i64"),
            InspectType::F32 => write!(f, "f32"),
            InspectType::F64 => write!(f, "f64"),
        }
    }
}

/// Info about the pixel under the cursor.
pub struct CursorInfo {
    pub file_offset: usize,
    pub byte_value: u8,
    pub row: usize,
    pub col: usize,
    pub bit_index: Option<usize>, // bit within byte (0=MSB) in bit mode
}

/// A byte-range selection in the file.
#[derive(Clone, Copy)]
pub struct Selection {
    pub start: usize,
    pub end: usize, // inclusive
}
