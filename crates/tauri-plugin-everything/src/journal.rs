//! NTFS USN_RECORD_V2 decoding. The pipe returns a next-USN cursor followed by records.

use crate::Error;

pub(crate) const LINK_CHANGE: u32 = 0x0001_0000;
pub(crate) const REMOVE: u32 = 0x0000_1200;
pub(crate) const DIRECTORY: u32 = 0x10;

#[derive(Debug)]
pub(crate) struct Change {
    pub reference: u64,
    pub parent: u64,
    pub reason: u32,
    pub attributes: u32,
    pub name: String,
}

pub(crate) fn decode(data: &[u8]) -> Result<(u64, Vec<Change>), Error> {
    let invalid = Error::ServiceIncompatible;
    let next = read_u64(data, 0).ok_or(invalid)?;
    let mut rest = &data[8..];
    let mut changes = Vec::new();
    while !rest.is_empty() {
        let len = read_u32(rest, 0).ok_or(invalid)? as usize;
        if len < 60 || !len.is_multiple_of(8) || len > rest.len() {
            return Err(invalid);
        }
        let (record, remaining) = rest.split_at(len);
        rest = remaining;
        if read_u16(record, 4) != Some(2) {
            return Err(Error::SnapshotUnsupported);
        }
        let name_len = usize::from(read_u16(record, 56).ok_or(invalid)?);
        let name_offset = usize::from(read_u16(record, 58).ok_or(invalid)?);
        if name_len % 2 != 0 || name_offset < 60 || name_offset % 2 != 0 {
            return Err(invalid);
        }
        let name_bytes = record
            .get(name_offset..name_offset + name_len)
            .ok_or(invalid)?;
        let name = utf16(name_bytes)?;
        changes.push(Change {
            reference: read_u64(record, 8).ok_or(invalid)?,
            parent: read_u64(record, 16).ok_or(invalid)?,
            reason: read_u32(record, 40).ok_or(invalid)?,
            attributes: read_u32(record, 52).ok_or(invalid)?,
            name,
        });
    }
    Ok((next, changes))
}

pub(crate) fn utf16(bytes: &[u8]) -> Result<String, Error> {
    let (pairs, remainder) = bytes.as_chunks::<2>();
    if !remainder.is_empty() {
        return Err(Error::ServiceIncompatible);
    }
    let words: Vec<_> = pairs.iter().copied().map(u16::from_le_bytes).collect();
    String::from_utf16(&words).map_err(|_| Error::SnapshotUnsupported)
}

pub(crate) fn read_u64(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
}

pub(crate) fn read_u32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn read_u16(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_unicode_rename_record_and_rejects_truncation() {
        let mut data = 300_u64.to_le_bytes().to_vec();
        let mut record = [0; 72];
        record[..4].copy_from_slice(&72_u32.to_le_bytes());
        record[4..6].copy_from_slice(&2_u16.to_le_bytes());
        record[8..16].copy_from_slice(&9_u64.to_le_bytes());
        record[16..24].copy_from_slice(&5_u64.to_le_bytes());
        record[40..44].copy_from_slice(&0x2000_u32.to_le_bytes());
        record[56..58].copy_from_slice(&4_u16.to_le_bytes());
        record[58..60].copy_from_slice(&60_u16.to_le_bytes());
        for (index, word) in "报告".encode_utf16().enumerate() {
            record[60 + index * 2..62 + index * 2].copy_from_slice(&word.to_le_bytes());
        }
        data.extend(record);
        let (next, changes) = decode(&data).unwrap();
        assert_eq!(next, 300);
        assert_eq!(changes[0].name, "报告");
        assert_eq!(changes[0].reference, 9);
        assert_eq!(changes[0].parent, 5);
        data.pop();
        assert!(decode(&data).is_err());
    }
}
