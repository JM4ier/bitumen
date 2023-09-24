/// Indicates that the archived object is a physical file.
pub const FILE: u32 = 0x0;

/// Indicates that the archived object is a directory
pub const DIR: u32 = 0x1;

/// Indicates that the archived object is a soft link
pub const SOFT_LINK: u32 = 0x2;

/// Indicates that the archived object is a soft link
pub const HARD_LINK: u32 = 0x3;

/// Indicates that the metadata is the header of the object.
/// If this bit is unset this means it is the footer.
pub const HEADER: u32 = 0x8;
