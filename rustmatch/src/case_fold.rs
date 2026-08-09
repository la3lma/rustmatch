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
    debug_assert!(table_is_valid(TABLE));
    lower_upper_from(TABLE, symbol)
}

fn lower_upper_from(table: &[u8], symbol: u16) -> (u16, u16) {
    let offset = HEADER_BYTES + usize::from(symbol) * BYTES_PER_ENTRY;
    (
        u16::from_be_bytes([table[offset], table[offset + 1]]),
        u16::from_be_bytes([table[offset + 2], table[offset + 3]]),
    )
}

fn table_is_valid(table: &[u8]) -> bool {
    table.len() == TABLE_BYTES
        && table.get(..4) == Some(b"RMF1")
        && table
            .get(4..8)
            .and_then(|header| header.try_into().ok())
            .is_some_and(|header| u32::from_be_bytes(header) == 65_536)
        && lower_upper_from(table, u16::from(b'a')) == (u16::from(b'a'), u16::from(b'A'))
        && lower_upper_from(table, 0x03a9) == (0x03c9, 0x03a9)
        && lower_upper_from(table, 0xd800) == (0xd800, 0xd800)
        && lower_upper_from(table, u16::MAX) == (u16::MAX, u16::MAX)
}

#[cfg(test)]
mod tests {
    use super::{ENTRY_COUNT, HEADER_BYTES, TABLE, TABLE_BYTES, lower_upper, table_is_valid};

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

    #[test]
    fn byte_swapped_table_is_rejected() {
        let mut corrupted = TABLE.to_vec();
        for encoded_unit in corrupted[HEADER_BYTES..].chunks_exact_mut(2) {
            encoded_unit.swap(0, 1);
        }

        assert!(table_is_valid(TABLE));
        assert!(!table_is_valid(&corrupted));
    }
}
