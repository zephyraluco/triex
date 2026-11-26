use std::{os::windows::ffi::OsStrExt, time::Duration};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::windows::named_pipe::{ClientOptions, NamedPipeClient},
};
use windows::{
    Win32::Storage::FileSystem::{
        GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW, GetVolumeNameForVolumeMountPointW,
    },
    core::PCWSTR,
};

use crate::{
    Error,
    journal::{self, Change},
    snapshot::{Snapshot, Volume},
    tree::{File, Folder, Tree},
};

const PIPE: &str = r"\\.\PIPE\Everything Service";
const IO_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_REPLY: usize = 64 * 1024 + 8;

pub(crate) async fn updates(volume: &Volume) -> Result<(u64, Vec<Change>), Error> {
    verify_drive_mapping(volume)?;
    let mut service = Service::open_volume(&volume.guid).await?;
    let journal_info = service.request(0x10, &[]).await?;
    let head = check_journal(&journal_info, volume)?;
    let mut cursor = volume.start_usn;
    let mut changes = Vec::new();
    for _ in 0..512 {
        if cursor >= head {
            return Ok((cursor, changes));
        }
        let mut request = [0; 40];
        request[..8].copy_from_slice(&cursor.to_le_bytes());
        request[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        request[32..40].copy_from_slice(&volume.journal_id.to_le_bytes());
        let response = service.request(0x12, &request).await?;
        let (next, batch) = journal::decode(&response)?;
        if next <= cursor {
            return Err(Error::SnapshotStale);
        }
        cursor = next;
        changes.extend(batch);
    }
    // The cursor and the changes are returned together. Neither is applied on a failed batch.
    Ok((cursor, changes))
}

pub(crate) async fn rebuild() -> Result<Snapshot, Error> {
    let drives = unsafe { GetLogicalDrives() };
    let mut volumes = Vec::new();
    let mut trees = Vec::new();
    for index in 0..26 {
        if drives & (1 << index) == 0 {
            continue;
        }
        let drive = format!("{}:", char::from(b'A' + index as u8));
        let path: Vec<_> = format!("{drive}\\").encode_utf16().chain([0]).collect();
        let mut fs_name = [0; 16];
        let root = PCWSTR(path.as_ptr());
        if unsafe { GetDriveTypeW(root) } != 3
            || unsafe { GetVolumeInformationW(root, None, None, None, None, Some(&mut fs_name)) }
                .is_err()
            || fs_name[..5] != [78, 84, 70, 83, 0]
        {
            continue;
        }
        let guid = mapped_guid(&drive)?;
        let mut service = Service::open_volume(&guid).await?;
        let journal = service.request(0x10, &[]).await?;
        if journal.len() != 56 {
            return Err(Error::ServiceIncompatible);
        }
        let volume = Volume {
            guid,
            drive,
            journal_id: journal::read_u64(&journal, 0).ok_or(Error::ServiceIncompatible)?,
            start_usn: journal::read_u64(&journal, 16).ok_or(Error::ServiceIncompatible)?,
        };
        if service.request(0x13, &[]).await?.len() != 32
            || !service
                .request(0x14, &0x1f_u32.to_le_bytes())
                .await?
                .is_empty()
        {
            return Err(Error::ServiceIncompatible);
        }
        let mut records = Vec::new();
        loop {
            let reply = service.request(0x15, &[]).await?;
            if reply.is_empty() {
                break;
            }
            decode_mft(&reply, &mut records)?;
        }
        trees.push(tree_from_mft(&volume.drive, records)?);
        volumes.push(volume);
    }
    if volumes.is_empty() {
        return Err(Error::BackendUnavailable);
    }
    Ok(Snapshot { volumes, trees })
}

#[derive(Debug)]
struct MftRecord {
    first: u64,
    second: u64,
    attributes: u32,
    name: String,
}

fn decode_mft(mut data: &[u8], records: &mut Vec<MftRecord>) -> Result<(), Error> {
    let invalid = Error::ServiceIncompatible;
    while !data.is_empty() {
        let len = usize::from(*data.get(52).ok_or(invalid)?) * 2;
        let (record, rest) = data.split_at_checked(53 + len).ok_or(invalid)?;
        data = rest;
        records.push(MftRecord {
            first: journal::read_u64(record, 0).ok_or(invalid)?,
            second: journal::read_u64(record, 8).ok_or(invalid)?,
            attributes: journal::read_u32(record, 48).ok_or(invalid)?,
            name: journal::utf16(&record[53..])?,
        });
    }
    Ok(())
}

fn tree_from_mft(drive: &str, records: Vec<MftRecord>) -> Result<Tree, Error> {
    use std::os::windows::fs::MetadataExt;
    const FRN_MASK: u64 = (1 << 48) - 1;
    // The root is NTFS record 5. A direct child's parent reference also
    // identifies the root if the service omits the root record itself.
    let is_root = |id: u64| id & FRN_MASK == 5;
    let mut root_record = None;
    let mut root_link = None;
    for record in &records {
        let link = match (is_root(record.first), is_root(record.second)) {
            (true, true) => {
                root_record = Some(record.first);
                continue;
            }
            (false, true) => (record.second, true),
            (true, false) => (record.first, false),
            (false, false) => continue,
        };
        if root_link.is_some_and(|previous| previous != link) {
            return Err(Error::SnapshotUnsupported);
        }
        root_link = Some(link);
    }
    let (root, first_is_id) = root_link
        .or_else(|| root_record.map(|root| (root, true)))
        .ok_or(Error::SnapshotUnsupported)?;
    if root_record.is_some_and(|record| record != root) {
        return Err(Error::SnapshotUnsupported);
    }
    let ids = |r: &MftRecord| {
        if first_is_id {
            (r.first, r.second)
        } else {
            (r.second, r.first)
        }
    };
    let mut tree = Tree::default();
    tree.folders.insert(
        root,
        Folder {
            parent: None,
            name: drive.to_owned(),
            modified: 0,
        },
    );
    for record in &records {
        let (id, parent) = ids(record);
        if id & FRN_MASK == 5 || record.attributes & journal::DIRECTORY == 0 {
            continue;
        }
        crate::snapshot::check_name(&record.name)?;
        tree.folders.entry(id).or_insert_with(|| Folder {
            parent: Some(parent),
            name: record.name.clone(),
            modified: 0,
        });
    }
    let paths = tree.paths(drive)?;
    for (&id, folder) in &mut tree.folders {
        folder.modified = std::fs::metadata(&paths[&id]).map_or(0, |m| m.last_write_time());
    }
    for record in records {
        if record.attributes & journal::DIRECTORY != 0 {
            continue;
        }
        let (_, parent) = ids(&record);
        crate::snapshot::check_name(&record.name)?;
        let directory = paths.get(&parent).ok_or(Error::SnapshotStale)?;
        let path = crate::snapshot::join(directory, &record.name)?;
        let metadata = std::fs::metadata(path).ok();
        tree.files.entry(parent).or_default().insert(
            record.name,
            File {
                size: metadata.as_ref().map_or(u64::MAX, |m| m.file_size()),
                modified: metadata.map_or(0, |m| m.last_write_time()),
            },
        );
    }
    Ok(tree)
}

fn verify_drive_mapping(volume: &Volume) -> Result<(), Error> {
    if !mapped_guid(&volume.drive)?.eq_ignore_ascii_case(&volume.guid) {
        return Err(Error::SnapshotStale);
    }
    Ok(())
}

fn mapped_guid(drive: &str) -> Result<String, Error> {
    let path: Vec<_> = std::ffi::OsStr::new(&format!("{drive}\\"))
        .encode_wide()
        .chain([0])
        .collect();
    let mut name = [0; 64];
    unsafe { GetVolumeNameForVolumeMountPointW(PCWSTR(path.as_ptr()), &mut name) }
        .map_err(|_| Error::SnapshotStale)?;
    let len = name
        .iter()
        .position(|&word| word == 0)
        .ok_or(Error::SnapshotStale)?;
    let name = String::from_utf16(&name[..len]).map_err(|_| Error::SnapshotStale)?;
    let guid = name.trim_end_matches('\\');
    if !crate::snapshot::valid_volume_guid(guid) {
        return Err(Error::SnapshotStale);
    }
    Ok(guid.to_owned())
}

fn check_journal(journal: &[u8], volume: &Volume) -> Result<u64, Error> {
    if journal.len() != 56 {
        return Err(Error::ServiceIncompatible);
    }
    let mut fields = journal
        .as_chunks::<8>()
        .0
        .iter()
        .copied()
        .map(u64::from_le_bytes);
    let id = fields.next().expect("journal id");
    let first = fields.next().expect("first USN");
    let next = fields.next().expect("next USN");
    let lowest_valid = fields.next().expect("lowest valid USN");
    if id != volume.journal_id
        || volume.start_usn < first.max(lowest_valid)
        || volume.start_usn > next
    {
        return Err(Error::SnapshotStale);
    }
    Ok(next)
}

struct Service(NamedPipeClient);

impl Service {
    async fn open_volume(guid: &str) -> Result<Self, Error> {
        let mut service = Self::connect().await?;
        let mut name = guid.as_bytes().to_vec();
        name.push(0);
        if !service.request(1, &name).await?.is_empty() {
            return Err(Error::ServiceIncompatible);
        }
        Ok(service)
    }

    async fn connect() -> Result<Self, Error> {
        Self::connect_to(PIPE).await
    }

    async fn connect_to(path: &str) -> Result<Self, Error> {
        let mut service =
            Self(
                ClientOptions::new()
                    .open(path)
                    .map_err(|error| match error.raw_os_error() {
                        Some(2) => Error::NotRunning,
                        Some(231) => Error::Busy,
                        _ => Error::BackendUnavailable,
                    })?,
            );
        if service.request(0, &[]).await? != 8_u32.to_le_bytes() {
            return Err(Error::ServiceIncompatible);
        }
        Ok(service)
    }

    async fn request(&mut self, command: u32, payload: &[u8]) -> Result<Vec<u8>, Error> {
        tokio::time::timeout(IO_TIMEOUT, async {
            let mut request = Vec::with_capacity(8 + payload.len());
            request.extend_from_slice(&((payload.len() + 8) as u32).to_le_bytes());
            request.extend_from_slice(&command.to_le_bytes());
            request.extend_from_slice(payload);
            self.0
                .write_all(&request)
                .await
                .map_err(|_| Error::BackendUnavailable)?;
            let length = self
                .0
                .read_u32_le()
                .await
                .map_err(|_| Error::BackendUnavailable)? as usize;
            if !(8..=MAX_REPLY).contains(&length) {
                return Err(Error::ServiceIncompatible);
            }
            let status = self
                .0
                .read_u32_le()
                .await
                .map_err(|_| Error::BackendUnavailable)?;
            let mut body = vec![0; length - 8];
            self.0
                .read_exact(&mut body)
                .await
                .map_err(|_| Error::BackendUnavailable)?;
            match status {
                0 => Ok(body),
                5 if command == 0x15 && journal::read_u32(&body, 0) == Some(259) => Ok(vec![]),
                5 if journal::read_u32(&body, 0) == Some(1181) => Err(Error::SnapshotStale),
                5 if body.len() >= 4 => Err(Error::BackendUnavailable),
                _ => Err(Error::ServiceIncompatible),
            }
        })
        .await
        .map_err(|_| Error::Timeout)?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_mft_pages_and_reconstructs_the_parent_tree() {
        let mut body = Vec::new();
        for (id, parent, attrs, name) in [
            (5_u64, 5_u64, 16_u32, "."),
            (7, 5, 16, "报表"),
            (8, 7, 0, "季度.txt"),
        ] {
            body.extend(id.to_le_bytes());
            body.extend(parent.to_le_bytes());
            body.extend([0; 32]);
            body.extend(attrs.to_le_bytes());
            body.push(name.encode_utf16().count() as u8);
            body.extend(name.encode_utf16().flat_map(u16::to_le_bytes));
        }
        let mut records = Vec::new();
        decode_mft(&body, &mut records).unwrap();
        let tree_without_root = tree_from_mft("Q:", records.split_off(1)).unwrap();
        assert!(tree_without_root.files[&7].contains_key("季度.txt"));
        records.clear();
        decode_mft(&body, &mut records).unwrap();
        let tree = tree_from_mft("Q:", records).unwrap();
        assert_eq!(tree.paths("Q:").unwrap()[&7], r"Q:\报表");
        assert!(tree.files[&7].contains_key("季度.txt"));
        body.pop();
        assert!(decode_mft(&body, &mut Vec::new()).is_err());
    }

    #[test]
    fn exchanges_version_and_journal_frames_over_a_named_pipe() {
        tauri::async_runtime::block_on(async {
            let path = format!(r"\\.\pipe\triex-everything-test-{}", std::process::id());
            let mut server = tokio::net::windows::named_pipe::ServerOptions::new()
                .first_pipe_instance(true)
                .create(&path)
                .unwrap();
            let task = tauri::async_runtime::spawn(async move {
                server.connect().await.unwrap();
                for command in [0, 0x10] {
                    assert_eq!(server.read_u32_le().await.unwrap(), 8);
                    assert_eq!(server.read_u32_le().await.unwrap(), command);
                    let payload = if command == 0 {
                        8_u32.to_le_bytes().to_vec()
                    } else {
                        vec![0; 56]
                    };
                    server.write_u32_le(8 + payload.len() as u32).await.unwrap();
                    server.write_u32_le(0).await.unwrap();
                    server.write_all(&payload).await.unwrap();
                }
                let mut eof = [0];
                assert_eq!(server.read(&mut eof).await.unwrap(), 0);
            });
            let mut client = Service::connect_to(&path).await.unwrap();
            assert_eq!(client.request(0x10, &[]).await.unwrap(), vec![0; 56]);
            drop(client);
            task.await.unwrap();
        });
    }

    #[test]
    fn rejects_journal_reset_and_accepts_a_replayable_cursor() {
        let volume = Volume {
            guid: String::new(),
            drive: String::new(),
            journal_id: 10,
            start_usn: 100,
        };
        let journal = |fields: [u64; 7]| {
            fields
                .into_iter()
                .flat_map(u64::to_le_bytes)
                .collect::<Vec<_>>()
        };
        assert!(check_journal(&journal([10, 80, 100, 80, 0, 0, 0]), &volume).is_ok());
        assert!(matches!(
            check_journal(&journal([10, 120, 200, 120, 0, 0, 0]), &volume),
            Err(Error::SnapshotStale)
        ));
        assert!(matches!(
            check_journal(&journal([11, 80, 100, 80, 0, 0, 0]), &volume),
            Err(Error::SnapshotStale)
        ));
        assert!(matches!(
            check_journal(&[0; 12], &volume),
            Err(Error::ServiceIncompatible)
        ));
    }
}
