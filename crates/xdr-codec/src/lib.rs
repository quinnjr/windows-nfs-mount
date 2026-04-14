pub mod error;
pub mod primitives;
pub mod traits;
pub mod variable;

pub use error::XdrError;
pub use traits::{XdrDecode, XdrEncode};
pub use variable::Opaque;
pub use xdr_derive::{XdrDecode, XdrEncode};
