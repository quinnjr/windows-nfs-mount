pub mod data;
pub mod filehandle;
pub mod session;

// Operation number constants (RFC 8881 section 18).
pub const OP_ACCESS: u32 = 3;
pub const OP_CLOSE: u32 = 4;
pub const OP_COMMIT: u32 = 5;
pub const OP_GETATTR: u32 = 9;
pub const OP_GETFH: u32 = 10;
pub const OP_LOOKUP: u32 = 15;
pub const OP_OPEN: u32 = 18;
pub const OP_PUTFH: u32 = 22;
pub const OP_PUTROOTFH: u32 = 24;
pub const OP_READ: u32 = 25;
pub const OP_READDIR: u32 = 26;
pub const OP_REMOVE: u32 = 28;
pub const OP_RENAME: u32 = 29;
pub const OP_SETATTR: u32 = 34;
pub const OP_WRITE: u32 = 38;
pub const OP_EXCHANGE_ID: u32 = 42;
pub const OP_CREATE_SESSION: u32 = 43;
pub const OP_DESTROY_SESSION: u32 = 44;
pub const OP_SEQUENCE: u32 = 53;
pub const OP_RECLAIM_COMPLETE: u32 = 58;
