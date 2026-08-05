//! Java 21 single-`char` case mappings used by the compatibility contract.

const HEADER_BYTES: usize = 8;
const BYTES_PER_ENTRY: usize = 4;
const ENTRY_COUNT: usize = 65_536;
const TABLE_BYTES: usize = HEADER_BYTES + ENTRY_COUNT * BYTES_PER_ENTRY;
const TABLE: &[u8; TABLE_BYTES] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/data/java-21-case-fold-v1.bin"
));

pub(crate) fn lower_upper(symbol: u16) -> (u16, u16) {
    debug_assert_eq!(&TABLE[..4], b"RMF1");
    debug_assert_eq!(
        u32::from_be_bytes(TABLE[4..8].try_into().expect("table header")),
        65_536
    );
    let offset = HEADER_BYTES + usize::from(symbol) * BYTES_PER_ENTRY;
    (
        u16::from_be_bytes([TABLE[offset], TABLE[offset + 1]]),
        u16::from_be_bytes([TABLE[offset + 2], TABLE[offset + 3]]),
    )
}

#[cfg(test)]
mod tests {
    use super::{ENTRY_COUNT, TABLE, TABLE_BYTES, lower_upper};

    #[test]
    fn generated_table_covers_every_utf16_code_unit() {
        // Prepare / Test / Assert
        assert_eq!(TABLE.len(), TABLE_BYTES);
        assert_eq!(ENTRY_COUNT, 65_536);
        assert_eq!(
            lower_upper(u16::from(b'a')),
            (u16::from(b'a'), u16::from(b'A'))
        );
        assert_eq!(
            lower_upper(u16::from(b'A')),
            (u16::from(b'a'), u16::from(b'A'))
        );
        assert_eq!(lower_upper(0x03a9), (0x03c9, 0x03a9));
        assert_eq!(lower_upper(0xd800), (0xd800, 0xd800));
        assert_eq!(lower_upper(u16::MAX), (u16::MAX, u16::MAX));
    }
}
