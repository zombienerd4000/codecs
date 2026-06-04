pub mod bit;
pub mod lz;
pub mod huff;
pub mod codec;

pub use codec::{compress, decompress};
pub use lz::HashType;
