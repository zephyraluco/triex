use crate::{Error, ItemKind, SearchItem, index::Entry};

pub(crate) struct Volume {
    pub(crate) guid: String,
    pub(crate) drive: String,
    pub(crate) journal_id: u64,
    pub(crate) start_usn: u64,
}

pub(crate) struct Snapshot {
    pub(crate) volumes: Vec<Volume>,
    pub(crate) entries: Vec<Entry>,
}

type Result<T> = std::result::Result<T, Error>;
const INVALID: Error = Error::SnapshotUnsupported;

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let (value, rest) = self.0.split_at_checked(count).ok_or(INVALID)?;
        self.0 = rest;
        Ok(value)
    }

    fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| INVALID)?,
        ))
    }

    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().map_err(|_| INVALID)?,
        ))
    }

    fn compact(&mut self) -> Result<usize> {
        match self.byte()? {
            255 => Ok(self.u32()? as usize),
            n => Ok(usize::from(n)),
        }
    }

    fn string(&mut self) -> Result<String> {
        let size = self.compact()?;
        String::from_utf8(self.take(size)?.to_vec()).map_err(|_| INVALID)
    }

    fn name(&mut self, previous: &mut Vec<u8>) -> Result<String> {
        let count = self.compact()?;
        if count != 0 {
            let trim = self.compact()?;
            let keep = previous.len().checked_sub(trim).ok_or(INVALID)?;
            previous.truncate(keep);
            previous.extend_from_slice(self.take(count)?);
        }
        String::from_utf8(previous.clone()).map_err(|_| INVALID)
    }

    fn permutation(&mut self, count: usize) -> Result<()> {
        let mut seen = vec![false; count];
        for _ in 0..count {
            let value = seen.get_mut(self.u32()? as usize).ok_or(INVALID)?;
            if std::mem::replace(value, true) {
                return Err(INVALID);
            }
        }
        Ok(())
    }
}

struct Directory {
    name: String,
    modified: u64,
    path: Option<String>,
}

impl Snapshot {
    pub(crate) fn decode(data: &[u8]) -> Result<Self> {
        let mut reader = Reader(data);
        // Format and index flags independently verified against the uploaded NTFS fixture.
        if reader.take(4)? != b"ESDb" || reader.u32()? != 0x01070014 || reader.u32()? != 0x2505 {
            return Err(INVALID);
        }
        let directories = reader.u32()? as usize;
        let files = reader.u32()? as usize;
        let source_count = reader.compact()?;
        if source_count == 0
            || source_count > 26
            || directories
                .checked_add(files)
                .is_none_or(|count| count > data.len() / 17)
        {
            return Err(INVALID);
        }

        let mut volumes = Vec::with_capacity(source_count);
        for _ in 0..source_count {
            if reader.compact()? != 0 || reader.byte()? != 0 {
                return Err(INVALID);
            }
            let guid = reader.string()?;
            let drive = reader.string()?;
            let root = reader.string()?;
            let included = reader.string()?;
            if !valid_volume_guid(&guid)
                || !valid_drive(&drive)
                || !root.is_empty()
                || !included.is_empty()
            {
                return Err(INVALID);
            }
            let journal_id = reader.u64()?;
            let start_usn = reader.u64()?;
            if journal_id == 0 || start_usn > i64::MAX as u64 {
                return Err(INVALID);
            }
            volumes.push(Volume {
                guid,
                drive,
                journal_id,
                start_usn,
            });
        }
        if reader.take(4)? != [0; 4] {
            return Err(INVALID);
        }

        let parents = (0..directories)
            .map(|_| reader.u32().map(|value| value as usize))
            .collect::<Result<Vec<_>>>()?;
        let mut previous = Vec::new();
        let mut folders = Vec::with_capacity(directories);
        for &parent in &parents {
            let name = reader.name(&mut previous)?;
            let modified = reader.u64()?;
            let _file_reference = reader.u64()?;
            let path = if parent >= directories {
                let volume = volumes.get(parent - directories).ok_or(INVALID)?;
                if !name.eq_ignore_ascii_case(&volume.drive) {
                    return Err(INVALID);
                }
                Some(format!("{name}\\"))
            } else {
                check_name(&name)?;
                None
            };
            folders.push(Directory {
                name,
                modified,
                path,
            });
        }

        let mut visiting = vec![false; folders.len()];
        let mut chain = Vec::new();
        for index in 0..folders.len() {
            let mut current = index;
            while folders[current].path.is_none() {
                if std::mem::replace(&mut visiting[current], true) {
                    return Err(INVALID);
                }
                chain.push(current);
                current = parents[current];
            }
            while let Some(child) = chain.pop() {
                let parent = folders[current].path.as_deref().ok_or(INVALID)?;
                let child_path = join(parent, &folders[child].name)?;
                folders[child].path = Some(child_path);
                visiting[child] = false;
                current = child;
            }
        }

        let mut entries = Vec::with_capacity(directories + files);
        previous.clear();
        for _ in 0..files {
            let folder = folders.get(reader.u32()? as usize).ok_or(INVALID)?;
            let name = reader.name(&mut previous)?;
            check_name(&name)?;
            let path = join(folder.path.as_deref().ok_or(INVALID)?, &name)?;
            let size = reader.u64()?;
            let modified = reader.u64()?;
            entries.push(Entry::new(SearchItem {
                name,
                path,
                kind: ItemKind::File,
                size_bytes: (size != u64::MAX).then_some(size),
                modified_at: unix_millis(modified),
            }));
        }

        if reader.u32()? as usize != directories {
            return Err(INVALID);
        }
        reader.permutation(directories)?;
        if reader.u32()? != 0 {
            return Err(INVALID);
        }
        for count in [directories, directories, files, files, files] {
            reader.permutation(count)?;
        }
        if !reader.0.is_empty() {
            return Err(INVALID);
        }

        entries.extend(folders.into_iter().map(|folder| {
            Entry::new(SearchItem {
                name: folder.name,
                path: folder.path.expect("resolved directory path"),
                kind: ItemKind::Folder,
                size_bytes: None,
                modified_at: unix_millis(folder.modified),
            })
        }));
        Ok(Self { volumes, entries })
    }
}

fn valid_drive(value: &str) -> bool {
    matches!(value.as_bytes(), [letter, b':'] if letter.is_ascii_alphabetic())
}

fn valid_volume_guid(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 48
        && bytes[..11].eq_ignore_ascii_case(br"\\?\Volume{")
        && bytes.ends_with(b"}")
        && bytes[11..47].iter().enumerate().all(|(i, byte)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                *byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

fn check_name(value: &str) -> Result<()> {
    if matches!(value, "" | "." | "..") || value.contains(['\\', '/', ':', '\0']) {
        return Err(INVALID);
    }
    Ok(())
}

fn join(parent: &str, name: &str) -> Result<String> {
    if parent.len() + name.len() + 1 > 32_768 * 4 {
        return Err(INVALID);
    }
    Ok(format!("{}\\{name}", parent.trim_end_matches('\\')))
}

fn unix_millis(filetime: u64) -> Option<i64> {
    const EPOCH: i64 = 11_644_473_600_000;
    (filetime != 0 && filetime != u64::MAX).then(|| (filetime / 10_000) as i64 - EPOCH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_truncated_or_wrong_format() {
        assert!(Snapshot::decode(b"ESDb").is_err());
        assert!(Snapshot::decode(b"not an Everything snapshot").is_err());
    }

    // Run with TRIEX_TEST_NTFS_DB pointing to an original 1.4.1.1032 NTFS snapshot.
    #[test]
    #[ignore = "requires an external NTFS fixture"]
    fn decodes_reference_ntfs_snapshot() {
        let path = std::env::var_os("TRIEX_TEST_NTFS_DB").expect("fixture path");
        let bytes = std::fs::read(path).unwrap();
        let snapshot = Snapshot::decode(&bytes).unwrap();
        assert_eq!(snapshot.volumes.len(), 3);
        assert_eq!(snapshot.entries.len(), 1_570_892);
        assert!(snapshot.volumes.iter().all(|volume| volume.start_usn > 0));
    }
}
