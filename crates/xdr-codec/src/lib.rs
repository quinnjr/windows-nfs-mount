pub mod error;
pub mod primitives;
pub mod traits;

pub use error::XdrError;
pub use traits::{XdrDecode, XdrEncode};
pub use xdr_derive::{XdrDecode, XdrEncode};
