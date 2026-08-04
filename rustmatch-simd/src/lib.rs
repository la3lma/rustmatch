//! Audited architecture-specific candidate-discovery kernels.

#![deny(unsafe_op_in_unsafe_fn)]

const FIVE_HASH_BITS: usize = 1 << 20;
const FIVE_HASH_WORDS: usize = FIVE_HASH_BITS / 64;

/// Reports whether the current process can execute the AVX2 candidate kernel.
#[must_use]
pub fn avx2_available() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        std::arch::is_x86_feature_detected!("avx2")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

/// Scans exact five-unit FNV prefix hashes into candidate bitmap words.
///
/// Returns `None` when AVX2 is unavailable or the hash table has an unexpected
/// size. Any five-unit window containing a non-ASCII unit is admitted
/// conservatively. Starts with fewer than five remaining units are rejected.
pub fn scan_five_hash_words(
    input: &[u16],
    hash_words: &[u64],
    first_start: usize,
    output_words: &mut [u64],
) -> Option<usize> {
    if !avx2_available() || hash_words.len() != FIVE_HASH_WORDS {
        return None;
    }
    #[cfg(target_arch = "x86_64")]
    {
        // SAFETY: AVX2 was detected above. The kernel checks all slice bounds,
        // and the safe wrapper validates the fixed table size it indexes.
        Some(unsafe { scan_five_hash_words_avx2(input, hash_words, first_start, output_words) })
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = (input, hash_words, first_start, output_words);
        None
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn scan_five_hash_words_avx2(
    input: &[u16],
    hash_words: &[u64],
    first_start: usize,
    output_words: &mut [u64],
) -> usize {
    let end_start = first_start
        .saturating_add(output_words.len().saturating_mul(64))
        .min(input.len());
    let mut admissions = 0_usize;
    for (word_offset, output) in output_words.iter_mut().enumerate() {
        let start = first_start + word_offset * 64;
        let valid_starts = end_start.saturating_sub(start).min(64);
        let mut candidates = 0_u64;
        let mut lane_start = 0_usize;
        while lane_start + 8 <= valid_starts && start + lane_start + 12 <= input.len() {
            // SAFETY: the loop proves that all five overlapping eight-lane
            // loads are in bounds, and this function requires AVX2.
            let lane_mask = unsafe { five_lane_mask_avx2(hash_words, input, start + lane_start) };
            candidates |= u64::from(lane_mask) << lane_start;
            lane_start += 8;
        }
        while lane_start < valid_starts {
            if scalar_five_allows(hash_words, input, start + lane_start) {
                candidates |= 1_u64 << lane_start;
            }
            lane_start += 1;
        }
        *output = candidates;
        admissions = admissions.saturating_add(
            usize::try_from(candidates.count_ones()).expect("u32 population count fits usize"),
        );
    }
    admissions
}

fn scalar_five_allows(hash_words: &[u64], input: &[u16], start: usize) -> bool {
    let Some(prefix) = input.get(start..start.saturating_add(5)) else {
        return false;
    };
    if prefix.iter().any(|&unit| unit >= 128) {
        return true;
    }
    let bit = five_hash_index(prefix);
    hash_words[bit / 64] & (1_u64 << (bit % 64)) != 0
}

fn five_hash_index(prefix: &[u16]) -> usize {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for &unit in prefix {
        hash ^= u64::from(unit);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    usize::try_from(hash & (FIVE_HASH_BITS as u64 - 1)).expect("hash index fits usize")
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[allow(clippy::cast_ptr_alignment)]
unsafe fn five_lane_mask_avx2(hash_words: &[u64], input: &[u16], start: usize) -> u8 {
    use std::arch::x86_64::{
        __m128i, _mm_loadu_si128, _mm256_and_si256, _mm256_castsi256_ps, _mm256_cmpeq_epi32,
        _mm256_cmpgt_epi32, _mm256_cvtepu16_epi32, _mm256_i32gather_epi32, _mm256_movemask_ps,
        _mm256_mullo_epi32, _mm256_or_si256, _mm256_set1_epi32, _mm256_srli_epi32,
        _mm256_srlv_epi32, _mm256_xor_si256,
    };

    const FNV_OFFSET_LOW: i32 = -2_078_137_563;
    const FNV_PRIME_LOW: i32 = 0x0000_01b3;
    const HASH_MASK: i32 = 0x000f_ffff;

    // SAFETY: the caller proves start..start+12 is in bounds. Each load reads
    // eight u16 values, with the final load beginning at start+4.
    let load = |offset: usize| unsafe {
        _mm256_cvtepu16_epi32(_mm_loadu_si128(
            input.as_ptr().add(start + offset).cast::<__m128i>(),
        ))
    };
    let units = [load(0), load(1), load(2), load(3), load(4)];
    let mut hash = _mm256_set1_epi32(FNV_OFFSET_LOW);
    let prime = _mm256_set1_epi32(FNV_PRIME_LOW);
    let mut combined = _mm256_set1_epi32(0);
    for unit in units {
        combined = _mm256_or_si256(combined, unit);
        hash = _mm256_mullo_epi32(_mm256_xor_si256(hash, unit), prime);
    }

    let bit_indices = _mm256_and_si256(hash, _mm256_set1_epi32(HASH_MASK));
    let word_indices = _mm256_srli_epi32::<5>(bit_indices);
    let shifts = _mm256_and_si256(bit_indices, _mm256_set1_epi32(31));
    // SAFETY: HASH_MASK confines every gather to the validated 2^20-bit table.
    // Reading u64 storage as native-endian u32 words preserves x86 bit order.
    let selected_words =
        unsafe { _mm256_i32gather_epi32::<4>(hash_words.as_ptr().cast::<i32>(), word_indices) };
    let selected_bits = _mm256_and_si256(
        _mm256_srlv_epi32(selected_words, shifts),
        _mm256_set1_epi32(1),
    );
    let matches = _mm256_cmpeq_epi32(selected_bits, _mm256_set1_epi32(1));
    let non_ascii = _mm256_cmpgt_epi32(combined, _mm256_set1_epi32(127));
    u8::try_from(_mm256_movemask_ps(_mm256_castsi256_ps(_mm256_or_si256(
        matches, non_ascii,
    ))))
    .expect("eight AVX2 lanes fit one byte")
}

#[cfg(test)]
mod tests {
    use super::{FIVE_HASH_WORDS, five_hash_index, scalar_five_allows, scan_five_hash_words};

    #[test]
    fn unavailable_kernel_returns_none_without_writing() {
        if super::avx2_available() {
            return;
        }
        let input = vec![u16::from(b'x'); 64];
        let hash_words = vec![0_u64; FIVE_HASH_WORDS];
        let mut output = [u64::MAX];

        assert_eq!(
            scan_five_hash_words(&input, &hash_words, 0, &mut output),
            None
        );
        assert_eq!(output, [u64::MAX]);
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn avx2_kernel_matches_exact_five_hash_and_non_ascii_rules() {
        if !super::avx2_available() {
            return;
        }
        let mut hash_words = vec![0_u64; FIVE_HASH_WORDS];
        let literal = [
            u16::from(b'n'),
            u16::from(b'e'),
            u16::from(b'e'),
            u16::from(b'd'),
            u16::from(b'l'),
        ];
        let bit = five_hash_index(&literal);
        hash_words[bit / 64] |= 1_u64 << (bit % 64);
        let mut input = vec![u16::from(b'x'); 130];
        input[9..14].copy_from_slice(&literal);
        input[71] = 0x100;
        let mut output = [0_u64; 3];

        let count =
            scan_five_hash_words(&input, &hash_words, 0, &mut output).expect("AVX2 is available");

        assert_eq!(count, 6);
        assert_ne!(output[0] & (1_u64 << 9), 0);
        for start in 67..=71 {
            assert_ne!(output[start / 64] & (1_u64 << (start % 64)), 0);
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn avx2_kernel_matches_scalar_oracle_across_offsets_and_tails() {
        if !super::avx2_available() {
            return;
        }
        let mut state = 0x9e37_79b9_u32;
        let mut input = Vec::with_capacity(257);
        for _ in 0..257 {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            input.push(u16::try_from(32 + state % 95).expect("ASCII unit fits u16"));
        }
        for index in [3, 64, 129, 250] {
            input[index] = 0x100 + u16::try_from(index).expect("test index fits u16");
        }

        let mut hash_words = vec![0_u64; FIVE_HASH_WORDS];
        for start in (0..input.len().saturating_sub(4)).step_by(11) {
            let prefix = &input[start..start + 5];
            if prefix.iter().all(|&unit| unit < 128) {
                let bit = five_hash_index(prefix);
                hash_words[bit / 64] |= 1_u64 << (bit % 64);
            }
        }

        for first_start in [0, 1, 7, 63, 64, 65, 127, 251, 257, 300] {
            for word_count in 0..=5 {
                let mut actual = vec![u64::MAX; word_count];
                let actual_count =
                    scan_five_hash_words(&input, &hash_words, first_start, &mut actual)
                        .expect("AVX2 is available");
                let mut expected = vec![0_u64; word_count];
                let end_start = first_start
                    .saturating_add(word_count.saturating_mul(64))
                    .min(input.len());
                let mut expected_count = 0_usize;
                for start in first_start..end_start {
                    if scalar_five_allows(&hash_words, &input, start) {
                        let relative = start - first_start;
                        expected[relative / 64] |= 1_u64 << (relative % 64);
                        expected_count += 1;
                    }
                }
                assert_eq!(
                    actual, expected,
                    "first_start={first_start}, words={word_count}"
                );
                assert_eq!(
                    actual_count, expected_count,
                    "first_start={first_start}, words={word_count}"
                );
            }
        }
    }
}
