//! Only the native ELF image's declared interpreter gains executable access.
use super::port::Refusal;
use std::ffi::CStr;

/// Read a little-endian native header number without unchecked indexing.
fn number(bytes: &[u8], offset: usize, length: usize) -> Result<u64, Refusal> {
    let field = bytes
        .get(offset..offset.checked_add(length).ok_or(Refusal::LaunchPin)?)
        .ok_or(Refusal::LaunchPin)?;
    let mut value = 0_u64;
    for (index, byte) in field.iter().enumerate() {
        value |= u64::from(*byte) << (index * 8);
    }
    Ok(value)
}
/// This qualified adapter accepts only little-endian ELF64 x86-64 executables.
pub(super) fn interpreter(bytes: &[u8]) -> Result<Option<String>, Refusal> {
    if !bytes.starts_with(b"\x7fELF\x02\x01\x01") || number(bytes, 18, 2)? != 62 {
        return Err(Refusal::LaunchPin);
    }
    let table = usize::try_from(number(bytes, 32, 8)?).map_err(|_| Refusal::LaunchPin)?;
    let size = usize::try_from(number(bytes, 54, 2)?).map_err(|_| Refusal::LaunchPin)?;
    let count = usize::try_from(number(bytes, 56, 2)?).map_err(|_| Refusal::LaunchPin)?;
    if table < 64 || size != 56 || count == 0 {
        return Err(Refusal::LaunchPin);
    }
    let table_end = table
        .checked_add(count.checked_mul(size).ok_or(Refusal::LaunchPin)?)
        .ok_or(Refusal::LaunchPin)?;
    bytes.get(table..table_end).ok_or(Refusal::LaunchPin)?;
    let mut stack = false;
    let mut found = None;
    for index in 0..count {
        let base = table
            .checked_add(index.checked_mul(size).ok_or(Refusal::LaunchPin)?)
            .ok_or(Refusal::LaunchPin)?;
        let header = bytes
            .get(base..base.checked_add(size).ok_or(Refusal::LaunchPin)?)
            .ok_or(Refusal::LaunchPin)?;
        let kind = number(header, 0, 4)?;
        let flags = number(header, 4, 4)?;
        // Exec's kernel-created mappings bypass the syscall filter.
        if kind == 1 && flags & 2 != 0 && flags & 1 != 0 {
            return Err(Refusal::LaunchPin);
        }
        if kind == 0x6474_e551 {
            if stack || flags & 1 != 0 {
                return Err(Refusal::LaunchPin);
            }
            stack = true;
        }
        if kind != 3 {
            continue;
        }
        if found.is_some() {
            return Err(Refusal::LaunchPin);
        }
        let offset = usize::try_from(number(header, 8, 8)?).map_err(|_| Refusal::LaunchPin)?;
        let length = usize::try_from(number(header, 32, 8)?).map_err(|_| Refusal::LaunchPin)?;
        if offset < table_end {
            return Err(Refusal::LaunchPin);
        }
        let path = CStr::from_bytes_with_nul(
            bytes
                .get(offset..offset.checked_add(length).ok_or(Refusal::LaunchPin)?)
                .ok_or(Refusal::LaunchPin)?,
        )
        .map_err(|_| Refusal::LaunchPin)?
        .to_str()
        .map_err(|_| Refusal::LaunchPin)?;
        if !path.starts_with('/') {
            return Err(Refusal::LaunchPin);
        }
        found = Some(path.trim_start_matches('/').to_owned());
    }
    if !stack {
        return Err(Refusal::LaunchPin);
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::{Refusal, interpreter, number};

    fn put(bytes: &mut [u8], offset: usize, field: &[u8]) {
        bytes
            .get_mut(offset..offset + field.len())
            .unwrap()
            .copy_from_slice(field);
    }
    /// Public synthetic ELF metadata only; not executable code.
    fn image(dynamic: bool) -> Vec<u8> {
        let mut bytes = vec![0; 176];
        put(&mut bytes, 0, b"\x7fELF\x02\x01\x01");
        put(&mut bytes, 18, &62_u16.to_le_bytes());
        put(&mut bytes, 32, &64_u64.to_le_bytes());
        put(&mut bytes, 54, &56_u16.to_le_bytes());
        put(&mut bytes, 56, &2_u16.to_le_bytes());
        put(&mut bytes, 64, &1_u32.to_le_bytes());
        put(&mut bytes, 68, &5_u32.to_le_bytes());
        put(&mut bytes, 120, &0x6474_e551_u32.to_le_bytes());
        put(&mut bytes, 124, &6_u32.to_le_bytes());
        if dynamic {
            bytes.resize(232, 0);
            put(&mut bytes, 56, &3_u16.to_le_bytes());
            put(&mut bytes, 176, &3_u32.to_le_bytes());
            put(&mut bytes, 184, &232_u64.to_le_bytes());
            put(&mut bytes, 208, &8_u64.to_le_bytes());
            bytes.extend_from_slice(b"/lib/ld\0");
        }
        bytes
    }
    #[test]
    fn n17_default_elf_native_header_and_checked_numbers() {
        assert_eq!(interpreter(&image(false)), Ok(None));
        assert_eq!(number(&[0x12, 0x34], 0, 2), Ok(0x3412));
        assert_eq!(number(&[], usize::MAX, 2), Err(Refusal::LaunchPin));
        assert_eq!(number(&[1], 0, 2), Err(Refusal::LaunchPin));
        assert_eq!(interpreter(&[]), Err(Refusal::LaunchPin));
        for (offset, value) in [
            (0, 0),
            (4, 1),
            (5, 2),
            (6, 0),
            (18, 1),
            (54, 55),
            (54, 57),
            (56, 0),
        ] {
            let mut malformed = image(false);
            put(&mut malformed, offset, &[value]);
            assert_eq!(interpreter(&malformed), Err(Refusal::LaunchPin));
        }
        let mut shifted = image(false);
        shifted.splice(64..64, [0; 8]);
        put(&mut shifted, 32, &72_u64.to_le_bytes());
        assert_eq!(interpreter(&shifted), Ok(None));
        let mut truncated = image(false);
        truncated.truncate(119);
        assert_eq!(interpreter(&truncated), Err(Refusal::LaunchPin));
        let mut overflow = image(false);
        put(&mut overflow, 32, &u64::MAX.to_le_bytes());
        assert_eq!(interpreter(&overflow), Err(Refusal::LaunchPin));
    }
    #[test]
    fn n17_default_elf_interpreter_bounds_shape_and_singleton() {
        let bytes = image(true);
        assert_eq!(interpreter(&bytes), Ok(Some("lib/ld".into())));
        for (offset, value) in [(232, b'x'), (233, 0xff), (239, b'x'), (234, 0)] {
            let mut malformed = bytes.clone();
            put(&mut malformed, offset, &[value]);
            assert_eq!(interpreter(&malformed), Err(Refusal::LaunchPin));
        }
        for (offset, value) in [(184, u64::MAX), (208, u64::MAX)] {
            let mut malformed = bytes.clone();
            put(&mut malformed, offset, &value.to_le_bytes());
            assert_eq!(interpreter(&malformed), Err(Refusal::LaunchPin));
        }
        let mut padded = bytes.clone();
        padded.splice(232..232, [0; 8]);
        put(&mut padded, 184, &240_u64.to_le_bytes());
        assert_eq!(interpreter(&padded), Ok(Some("lib/ld".into())));
        let mut short_table = image(false);
        short_table.resize(128, 0);
        put(&mut short_table, 32, &8_u64.to_le_bytes());
        put(&mut short_table, 8, &0x6474_e551_u32.to_le_bytes());
        put(&mut short_table, 12, &6_u32.to_le_bytes());
        put(&mut short_table, 54, &56_u16.to_le_bytes());
        put(&mut short_table, 56, &1_u16.to_le_bytes());
        assert_eq!(interpreter(&short_table), Err(Refusal::LaunchPin));
        let mut wrong_size = image(false);
        wrong_size.resize(128, 0);
        put(&mut wrong_size, 54, &64_u16.to_le_bytes());
        put(&mut wrong_size, 56, &1_u16.to_le_bytes());
        put(&mut wrong_size, 64, &0x6474_e551_u32.to_le_bytes());
        put(&mut wrong_size, 68, &6_u32.to_le_bytes());
        assert_eq!(interpreter(&wrong_size), Err(Refusal::LaunchPin));
        let header = bytes.get(176..232).unwrap();
        let mut duplicate = bytes.clone();
        duplicate.resize(296, 0);
        put(&mut duplicate, 56, &4_u16.to_le_bytes());
        for offset in [176, 232] {
            put(&mut duplicate, offset, header);
            put(&mut duplicate, offset + 8, &288_u64.to_le_bytes());
        }
        put(&mut duplicate, 288, b"/lib/ld\0");
        assert_eq!(interpreter(&duplicate), Err(Refusal::LaunchPin));
    }
    #[test]
    fn n17_default_elf_kernel_exec_wx_stack_and_overlapping_headers_refuse() {
        assert_eq!(interpreter(&image(false)), Ok(None));
        for (offset, field) in [(68, 7_u32), (124, 7), (120, 0)] {
            let mut malformed = image(false);
            put(&mut malformed, offset, &field.to_le_bytes());
            assert_eq!(interpreter(&malformed), Err(Refusal::LaunchPin));
        }
        let mut overlap = image(false);
        put(&mut overlap, 32, &63_u64.to_le_bytes());
        assert_eq!(interpreter(&overlap), Err(Refusal::LaunchPin));
        let mut payload_overlap = image(true);
        put(&mut payload_overlap, 184, &176_u64.to_le_bytes());
        assert_eq!(interpreter(&payload_overlap), Err(Refusal::LaunchPin));
        let mut duplicate = image(false);
        put(&mut duplicate, 64, &0x6474_e551_u32.to_le_bytes());
        put(&mut duplicate, 68, &6_u32.to_le_bytes());
        assert_eq!(interpreter(&duplicate), Err(Refusal::LaunchPin));
        for flags in [4_u32, 6] {
            let mut neighbour = image(false);
            put(&mut neighbour, 68, &flags.to_le_bytes());
            assert_eq!(interpreter(&neighbour), Ok(None));
        }
    }
}
