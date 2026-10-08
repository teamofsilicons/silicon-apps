//! Publish complete immutable objects without replacing an existing inode.
use std::{io, path::Path};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

struct Temporary(std::path::PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// The directory is private service-owned storage. A hard link atomically exposes
/// the completed temporary file and refuses to clobber a concurrent publisher.
pub async fn publish(destination: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = destination
        .parent()
        .ok_or_else(|| io::Error::other("Object has no directory"))?;
    let temporary = Temporary(parent.join(format!(".{}.tmp", uuid::Uuid::new_v4())));
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&temporary.0).await?;
    file.write_all(bytes).await?;
    file.sync_all().await?;
    drop(file);
    match tokio::fs::hard_link(&temporary.0, destination).await {
        Ok(()) => {
            #[cfg(unix)]
            tokio::fs::File::open(parent).await?.sync_all().await?;
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let metadata = tokio::fs::symlink_metadata(destination).await?;
            if !metadata.is_file() || metadata.len() != bytes.len() as u64 {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "Existing object differs",
                ));
            }
            let mut existing = tokio::fs::File::open(destination).await?;
            let mut buffer = [0; 65536];
            for chunk in bytes.chunks(buffer.len()) {
                existing.read_exact(&mut buffer[..chunk.len()]).await?;
                if &buffer[..chunk.len()] != chunk {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "Existing object differs",
                    ));
                }
            }
            Ok(())
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn concurrent_duplicate_publication_keeps_existing_inode_and_complete_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("object");
        let bytes = vec![42; 256 * 1024];
        let (first, second) = tokio::join!(publish(&path, &bytes), publish(&path, &bytes));
        first.unwrap();
        second.unwrap();
        assert_eq!(tokio::fs::read(&path).await.unwrap(), bytes);
        // A hard-linked snapshot must remain the very same object after retries.
        let snapshot = directory.path().join("snapshot");
        tokio::fs::hard_link(&path, &snapshot).await.unwrap();
        #[cfg(unix)]
        let inode = {
            use std::os::unix::fs::MetadataExt;
            std::fs::metadata(&path).unwrap().ino()
        };
        publish(&path, &bytes).await.unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(std::fs::metadata(&path).unwrap().ino(), inode);
        }
        assert_eq!(tokio::fs::read(&snapshot).await.unwrap(), bytes);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
    }

    #[tokio::test]
    async fn conflicting_content_does_not_replace_committed_object_or_leave_temporary_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("object");
        publish(&path, b"image/jpeg").await.unwrap();
        assert_eq!(
            publish(&path, b"video/mp4").await.unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(tokio::fs::read(&path).await.unwrap(), b"image/jpeg");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
