use std::{os::windows::ffi::OsStrExt, time::Duration};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::windows::named_pipe::{ClientOptions, NamedPipeClient},
};
use windows::{Win32::Storage::FileSystem::GetVolumeNameForVolumeMountPointW, core::PCWSTR};

use crate::{Error, snapshot::Volume};

const PIPE: &str = r"\\.\PIPE\Everything Service";
const IO_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_REPLY: usize = 64 * 1024 + 8;

pub(crate) async fn verify_checkpoints(volumes: &[Volume]) -> Result<(), Error> {
    for volume in volumes {
        verify_drive_mapping(volume)?;
        let mut service = Service::connect().await?;
        let mut name = volume.guid.as_bytes().to_vec();
        name.push(0);
        if !service.request(1, &name).await?.is_empty() {
            return Err(Error::ServiceIncompatible);
        }
        let journal = service.request(0x10, &[]).await?;
        check_journal(&journal, volume)?;
    }
    Ok(())
}

fn verify_drive_mapping(volume: &Volume) -> Result<(), Error> {
    let path: Vec<_> = std::ffi::OsStr::new(&format!("{}\\", volume.drive))
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
    let mapped = String::from_utf16(&name[..len]).map_err(|_| Error::SnapshotStale)?;
    if !mapped
        .trim_end_matches('\\')
        .eq_ignore_ascii_case(&volume.guid)
    {
        return Err(Error::SnapshotStale);
    }
    Ok(())
}

fn check_journal(journal: &[u8], volume: &Volume) -> Result<(), Error> {
    if journal.len() != 56 {
        return Err(Error::ServiceIncompatible);
    }
    let mut fields = journal
        .chunks_exact(8)
        .map(|bytes| u64::from_le_bytes(bytes.try_into().expect("8-byte USN field")));
    let id = fields.next().expect("journal id");
    let first = fields.next().expect("first USN");
    let next = fields.next().expect("next USN");
    let lowest_valid = fields.next().expect("lowest valid USN");
    if id != volume.journal_id
        || volume.start_usn < first.max(lowest_valid)
        || volume.start_usn != next
    {
        return Err(Error::SnapshotStale);
    }
    Ok(())
}

struct Service(NamedPipeClient);

impl Service {
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
    fn verifies_only_a_snapshot_at_the_current_journal_head() {
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
            check_journal(&journal([10, 80, 200, 80, 0, 0, 0]), &volume),
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
