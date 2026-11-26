use crate::{
    Error,
    index::Entry,
    tree::{File, Folder, Tree},
};

#[derive(Clone)]
pub(crate) struct Volume {
    pub(crate) guid: String,
    pub(crate) drive: String,
    pub(crate) journal_id: u64,
    pub(crate) start_usn: u64,
}

pub(crate) struct Snapshot {
    pub(crate) volumes: Vec<Volume>,
    pub(crate) trees: Vec<Tree>,
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
    frn: u64,
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
            let frn = reader.u64()?;
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
                frn,
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

        let mut trees: Vec<_> = volumes.iter().map(|_| Tree::default()).collect();
        let mut owners = Vec::with_capacity(folders.len());
        for (index, folder) in folders.iter().enumerate() {
            let path = folder.path.as_deref().ok_or(INVALID)?;
            let owner = volumes
                .iter()
                .position(|v| {
                    path.get(..2)
                        .is_some_and(|d| d.eq_ignore_ascii_case(&v.drive))
                })
                .ok_or(INVALID)?;
            owners.push(owner);
            let parent = folders.get(parents[index]).map(|dir| dir.frn);
            if trees[owner]
                .folders
                .insert(
                    folder.frn,
                    Folder {
                        name: folder.name.clone(),
                        modified: folder.modified,
                        parent,
                    },
                )
                .is_some()
            {
                return Err(INVALID);
            }
        }
        previous.clear();
        for _ in 0..files {
            let parent = reader.u32()? as usize;
            let folder = folders.get(parent).ok_or(INVALID)?;
            let name = reader.name(&mut previous)?;
            check_name(&name)?;
            let size = reader.u64()?;
            let modified = reader.u64()?;
            if trees[owners[parent]]
                .files
                .entry(folder.frn)
                .or_default()
                .insert(name, File { size, modified })
                .is_some()
            {
                return Err(INVALID);
            }
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

        Ok(Self { volumes, trees })
    }

    pub(crate) fn entries(&self) -> Result<Vec<Entry>> {
        let mut entries = Vec::new();
        for (volume, tree) in self.volumes.iter().zip(&self.trees) {
            entries.extend(tree.entries(&volume.drive)?);
        }
        Ok(entries)
    }
}

fn valid_drive(value: &str) -> bool {
    matches!(value.as_bytes(), [letter, b':'] if letter.is_ascii_alphabetic())
}

pub(crate) fn valid_volume_guid(value: &str) -> bool {
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

pub(crate) fn check_name(value: &str) -> Result<()> {
    if matches!(value, "" | "." | "..") || value.contains(['\\', '/', ':', '\0']) {
        return Err(INVALID);
    }
    Ok(())
}

pub(crate) fn join(parent: &str, name: &str) -> Result<String> {
    if parent.len() + name.len() + 1 > 32_768 * 4 {
        return Err(INVALID);
    }
    Ok(format!("{}\\{name}", parent.trim_end_matches('\\')))
}

pub(crate) fn unix_millis(filetime: u64) -> Option<i64> {
    const EPOCH: i64 = 11_644_473_600_000;
    (filetime != 0 && filetime != u64::MAX).then(|| (filetime / 10_000) as i64 - EPOCH)
}

impl Snapshot {
    pub(crate) fn write_to(&self, output: &mut impl std::io::Write) -> std::io::Result<()> {
        use std::io::{Error as IoError, ErrorKind};
        let invalid = || IoError::new(ErrorKind::InvalidData, "invalid Everything snapshot");
        let mut folders = Vec::new();
        let mut files = Vec::new();
        for (volume_number, (volume, tree)) in self.volumes.iter().zip(&self.trees).enumerate() {
            let paths = tree.paths(&volume.drive).map_err(|_| invalid())?;
            let mut ids: Vec<_> = tree.folders.keys().copied().collect();
            ids.sort_unstable_by(|a, b| paths[a].cmp(&paths[b]));
            let positions: std::collections::HashMap<_, _> = ids
                .iter()
                .enumerate()
                .map(|(index, &id)| (id, folders.len() + index))
                .collect();
            for id in ids {
                let folder = &tree.folders[&id];
                let parent = folder
                    .parent
                    .map(|parent| positions.get(&parent).copied().ok_or_else(invalid))
                    .transpose()?;
                folders.push((volume_number, parent, id, folder, paths[&id].clone()));
                if let Some(children) = tree.files.get(&id) {
                    for (name, file) in children {
                        files.push((positions[&id], name.as_str(), file, name.to_lowercase()));
                    }
                }
            }
        }
        files.sort_unstable_by(|a, b| {
            a.3.cmp(&b.3)
                .then_with(|| a.0.cmp(&b.0))
                .then_with(|| a.1.cmp(b.1))
        });
        output.write_all(b"ESDb")?;
        for number in [0x01070014, 0x2505, folders.len() as u32, files.len() as u32] {
            output.write_all(&number.to_le_bytes())?;
        }
        compact_write(output, self.volumes.len())?;
        for volume in &self.volumes {
            output.write_all(&[0, 0])?;
            for argument in [&volume.guid, &volume.drive] {
                compact_write(output, argument.len())?;
                output.write_all(argument.as_bytes())?;
            }
            output.write_all(&[0, 0])?;
            output.write_all(&volume.journal_id.to_le_bytes())?;
            output.write_all(&volume.start_usn.to_le_bytes())?;
        }
        output.write_all(&[0; 4])?;
        for (volume, parent, _, _, _) in &folders {
            let index = parent.unwrap_or(folders.len() + volume);
            output.write_all(&(index as u32).to_le_bytes())?;
        }
        let mut previous: &[u8] = &[];
        for (_, _, id, folder, _) in &folders {
            prefix_write(output, previous, folder.name.as_bytes())?;
            previous = folder.name.as_bytes();
            output.write_all(&folder.modified.to_le_bytes())?;
            output.write_all(&id.to_le_bytes())?;
        }
        previous = &[];
        for (parent, name, file, _) in &files {
            output.write_all(&(*parent as u32).to_le_bytes())?;
            prefix_write(output, previous, name.as_bytes())?;
            previous = name.as_bytes();
            output.write_all(&file.size.to_le_bytes())?;
            output.write_all(&file.modified.to_le_bytes())?;
        }
        output.write_all(&(folders.len() as u32).to_le_bytes())?;
        let folder_names: Vec<_> = folders.iter().map(|f| f.3.name.to_lowercase()).collect();
        let folder_paths: Vec<_> = folders.iter().map(|f| f.4.to_lowercase()).collect();
        permutation_write(output, folders.len(), |a, b| {
            folder_names[a].cmp(&folder_names[b])
        })?;
        output.write_all(&0_u32.to_le_bytes())?;
        permutation_write(output, folders.len(), |a, b| {
            folder_paths[a].cmp(&folder_paths[b])
        })?;
        permutation_write(output, folders.len(), |a, b| {
            folders[a].3.modified.cmp(&folders[b].3.modified)
        })?;
        permutation_write(output, files.len(), |a, b| {
            folder_paths[files[a].0].cmp(&folder_paths[files[b].0])
        })?;
        permutation_write(output, files.len(), |a, b| {
            (files[a].2.size as i64).cmp(&(files[b].2.size as i64))
        })?;
        permutation_write(output, files.len(), |a, b| {
            (files[a].2.modified as i64).cmp(&(files[b].2.modified as i64))
        })?;
        Ok(())
    }
}

fn permutation_write(
    output: &mut impl std::io::Write,
    count: usize,
    compare: impl Fn(usize, usize) -> std::cmp::Ordering,
) -> std::io::Result<()> {
    let mut order: Vec<u32> = (0..count as u32).collect();
    order.sort_unstable_by(|a, b| compare(*a as usize, *b as usize).then_with(|| a.cmp(b)));
    for id in order {
        output.write_all(&id.to_le_bytes())?;
    }
    Ok(())
}

fn compact_write(output: &mut impl std::io::Write, value: usize) -> std::io::Result<()> {
    if value < 255 {
        output.write_all(&[value as u8])
    } else {
        output.write_all(&[255])?;
        output.write_all(&(value as u32).to_le_bytes())
    }
}

fn prefix_write(
    output: &mut impl std::io::Write,
    previous: &[u8],
    name: &[u8],
) -> std::io::Result<()> {
    if previous == name {
        return output.write_all(&[0]);
    }
    let shared = previous
        .iter()
        .zip(name)
        .take_while(|(a, b)| a == b)
        .count()
        .min(name.len() - 1);
    compact_write(output, name.len() - shared)?;
    compact_write(output, previous.len() - shared)?;
    output.write_all(&name[shared..])
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
        assert_eq!(snapshot.entries().unwrap().len(), 1_570_892);
        assert!(snapshot.volumes.iter().all(|volume| volume.start_usn > 0));
        let mut encoded = Vec::new();
        snapshot.write_to(&mut encoded).unwrap();
        if let Some(path) = std::env::var_os("TRIEX_TEST_NTFS_OUTPUT") {
            std::fs::write(path, &encoded).unwrap();
        }
        let restored = Snapshot::decode(&encoded).unwrap();
        assert_eq!(restored.volumes.len(), 3);
        assert_eq!(restored.entries().unwrap().len(), 1_570_892);
        for (old, new) in snapshot.trees.iter().zip(&restored.trees) {
            assert!(old.folders == new.folders);
            assert!(old.files == new.files);
        }
        for (old, new) in snapshot.volumes.iter().zip(&restored.volumes) {
            assert_eq!(
                (&old.guid, old.journal_id, old.start_usn),
                (&new.guid, new.journal_id, new.start_usn)
            );
        }
    }
}
