use std::{
    collections::{HashMap, HashSet},
    os::windows::fs::MetadataExt,
    path::Path,
};

use crate::{
    Error, ItemKind, SearchItem,
    index::Entry,
    journal::{self, Change},
    snapshot::{self, Volume},
};

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Folder {
    pub parent: Option<u64>,
    pub name: String,
    pub modified: u64,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct File {
    pub size: u64,
    pub modified: u64,
}

#[derive(Default)]
pub(crate) struct Tree {
    pub folders: HashMap<u64, Folder>,
    pub files: HashMap<u64, HashMap<String, File>>,
}

impl Tree {
    pub(crate) fn paths(&self, drive: &str) -> Result<HashMap<u64, String>, Error> {
        let mut paths = HashMap::with_capacity(self.folders.len());
        let mut visiting = HashSet::new();
        let mut pending = Vec::new();
        for &start in self.folders.keys() {
            let mut id = start;
            while !paths.contains_key(&id) {
                let folder = self.folders.get(&id).ok_or(Error::SnapshotStale)?;
                if let Some(parent) = folder.parent {
                    if !visiting.insert(id) {
                        return Err(Error::SnapshotStale);
                    }
                    pending.push(id);
                    id = parent;
                } else {
                    paths.insert(id, format!("{drive}\\"));
                    break;
                }
            }
            while let Some(child) = pending.pop() {
                let child_path = snapshot::join(&paths[&id], &self.folders[&child].name)?;
                paths.insert(child, child_path);
                visiting.remove(&child);
                id = child;
            }
        }
        Ok(paths)
    }

    pub(crate) fn entries(&self, drive: &str) -> Result<Vec<Entry>, Error> {
        let paths = self.paths(drive)?;
        let mut entries = Vec::new();
        for (&id, folder) in &self.folders {
            entries.push(Entry::new(SearchItem {
                name: folder.name.clone(),
                path: paths[&id].clone(),
                kind: ItemKind::Folder,
                size_bytes: None,
                modified_at: snapshot::unix_millis(folder.modified),
            }));
        }
        for (&parent, files) in &self.files {
            let path = paths.get(&parent).ok_or(Error::SnapshotStale)?;
            for (name, file) in files {
                entries.push(Entry::new(SearchItem {
                    name: name.clone(),
                    path: snapshot::join(path, name)?,
                    kind: ItemKind::File,
                    size_bytes: (file.size != u64::MAX).then_some(file.size),
                    modified_at: snapshot::unix_millis(file.modified),
                }));
            }
        }
        Ok(entries)
    }

    pub(crate) fn replay(&mut self, volume: &Volume, changes: Vec<Change>) -> Result<(), Error> {
        let mut refresh = HashSet::new();
        for change in changes {
            if change.reason & journal::LINK_CHANGE != 0 {
                // USN reports the identity, not the complete replacement set of hard links.
                return Err(Error::SnapshotStale);
            }
            if change.attributes & journal::DIRECTORY != 0
                && self
                    .folders
                    .get(&change.reference)
                    .is_some_and(|dir| dir.parent.is_none())
            {
                continue;
            }
            snapshot::check_name(&change.name)?;
            if change.reason & 0x3000 == 0x3000 {
                return Err(Error::SnapshotStale);
            }
            if change.attributes & journal::DIRECTORY != 0 {
                if change.reason & 0x200 != 0 {
                    self.remove_folder(change.reference);
                } else if change.reason & 0x1000 == 0 {
                    // The old half of a rename leaves the subtree intact until the new half.
                    let modified = self
                        .folders
                        .get(&change.reference)
                        .map_or(0, |dir| dir.modified);
                    self.folders.insert(
                        change.reference,
                        Folder {
                            parent: Some(change.parent),
                            name: change.name,
                            modified,
                        },
                    );
                }
                continue;
            }

            if change.reason & journal::REMOVE != 0 {
                if let Some(files) = self.files.get_mut(&change.parent) {
                    files.remove(&change.name);
                }
                refresh.remove(&(change.parent, change.name));
            } else if change.reason & !0x8000_0000 != 0 {
                refresh.insert((change.parent, change.name));
            }
        }

        let paths = self.paths(&volume.drive)?;
        for (parent, name) in refresh {
            let Some(directory) = paths.get(&parent) else {
                return Err(Error::SnapshotStale);
            };
            let path = snapshot::join(directory, &name)?;
            match std::fs::metadata(Path::new(&path)) {
                Ok(metadata) if metadata.is_file() => {
                    self.files.entry(parent).or_default().insert(
                        name,
                        File {
                            size: metadata.file_size(),
                            modified: metadata.last_write_time(),
                        },
                    );
                }
                Ok(_) => return Err(Error::SnapshotStale),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    if let Some(files) = self.files.get_mut(&parent) {
                        files.remove(&name);
                    }
                }
                Err(_) => return Err(Error::SnapshotStale),
            }
        }
        Ok(())
    }

    fn remove_folder(&mut self, root: u64) {
        let mut children: HashMap<u64, Vec<u64>> = HashMap::new();
        for (&id, folder) in &self.folders {
            if let Some(parent) = folder.parent {
                children.entry(parent).or_default().push(id);
            }
        }
        let mut pending = vec![root];
        while let Some(id) = pending.pop() {
            self.folders.remove(&id);
            self.files.remove(&id);
            if let Some(children) = children.remove(&id) {
                pending.extend(children);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_directory_renames_and_removes_deleted_files() {
        let mut tree = Tree::default();
        let volume = Volume {
            guid: String::new(),
            drive: "C:".into(),
            journal_id: 1,
            start_usn: 0,
        };
        for (id, parent, name) in [
            (5, None, "C:"),
            (7, Some(5), "before"),
            (8, Some(7), "child"),
        ] {
            tree.folders.insert(
                id,
                Folder {
                    parent,
                    name: name.into(),
                    modified: 0,
                },
            );
        }
        tree.files.entry(8).or_default().insert(
            "remove.txt".into(),
            File {
                size: 0,
                modified: 0,
            },
        );
        tree.files.entry(8).or_default().insert(
            "keep.txt".into(),
            File {
                size: 0,
                modified: 0,
            },
        );
        tree.replay(
            &volume,
            vec![
                Change {
                    reference: 7,
                    parent: 5,
                    reason: 0x1000,
                    attributes: 0x10,
                    name: "before".into(),
                },
                Change {
                    reference: 7,
                    parent: 5,
                    reason: 0x2000,
                    attributes: 0x10,
                    name: "after".into(),
                },
                Change {
                    reference: 9,
                    parent: 8,
                    reason: 0x200,
                    attributes: 0,
                    name: "remove.txt".into(),
                },
            ],
        )
        .unwrap();
        assert_eq!(tree.paths("C:").unwrap()[&8], r"C:\after\child");
        assert!(!tree.files[&8].contains_key("remove.txt"));
        assert!(tree.files[&8].contains_key("keep.txt"));
    }
}
