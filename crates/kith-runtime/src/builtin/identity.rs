//! 16-byte ids, the same width as a UUID. Eight bytes of dispatcher time,
//! four of increment, and four random bytes from one World RNG step.

/// Low 32 bits of one World RNG step. Those bits are the id's random field.
pub fn random_bytes(state: &mut u64) -> u32 {
    (super::random::step(state) & 0xffff_ffff) as u32
}

pub fn text(timestamp: u64, increment: u32, random: u32) -> String {
    let mut bytes = [0u8; 16];
    bytes[0..8].copy_from_slice(&timestamp.to_be_bytes());
    bytes[8..12].copy_from_slice(&increment.to_be_bytes());
    bytes[12..16].copy_from_slice(&random.to_be_bytes());
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
