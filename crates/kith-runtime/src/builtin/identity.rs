//! ObjectId-shaped ids. The counter occupies the timestamp and counter
//! fields. The five middle bytes stay zero so a call does not draw entropy.

/// The last call count that still fits in the 4-byte and 3-byte fields.
pub const LAST: u64 = (1 << 56) - 1;

pub fn text(counter: u64) -> String {
    let high = (counter >> 24) as u32;
    let low = (counter & 0x00ff_ffff) as u32;
    let mut bytes = [0u8; 12];
    bytes[0..4].copy_from_slice(&high.to_be_bytes());
    bytes[9] = (low >> 16) as u8;
    bytes[10] = (low >> 8) as u8;
    bytes[11] = low as u8;
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
