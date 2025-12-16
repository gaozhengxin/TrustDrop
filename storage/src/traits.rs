// src/traits.rs (or whatever file you use)
// No tokio-util required.

use async_trait::async_trait;
use bytes::Bytes;
use std::path::Path;
use tokio::{ fs, io::{ self, AsyncRead, AsyncReadExt } };

use crate::{ BlobId, BlobStatus, ClientConfig, StorageError };

/// StorageNetwork trait split into 7 public functions as requested.
///
/// - load_file: read disk and return an AsyncRead (default: tokio::fs::File)
/// - upload_blob: upload bytes + optional extra parameter (e.g. epoch for Walrus) — must be implemented by provider
/// - get_status: query metadata/status — must be implemented by provider
/// - download: return an AsyncRead for the blob (must be implemented by provider)
/// - write_file: write an AsyncRead to disk (default: streaming copy)
/// - upload_file: convenience (default: load_file -> upload_blob)
/// - download_file: convenience (default: download -> write_file)
#[async_trait]
pub trait StorageNetwork: Send + Sync {
    /// Reconfigure client
    async fn configure(&mut self, cfg: ClientConfig);

    // ---------------------------------------------------------------------
    // 1. load_file: read disk file and return an AsyncRead boxed trait object.
    //    Default implementation returns a tokio::fs::File opened for reading.
    //    This is streaming (does NOT read whole file into memory).
    // ---------------------------------------------------------------------
    async fn load_file<P: AsRef<Path> + Send + Sync>(
        &self,
        path: P
    ) -> Result<Box<dyn AsyncRead + Send + Unpin>, StorageError> {
        let file = fs::File::open(path).await?;
        Ok(Box::new(file))
    }

    // ---------------------------------------------------------------------
    // 2. upload_blob: upload bytes. extra: optional string param for provider-specific usage
    //    (for Walrus you can use it to pass epoch). Implemented by provider.
    // ---------------------------------------------------------------------
    async fn upload_blob(&self, data: Bytes, extra: Option<&str>) -> Result<BlobId, StorageError>;

    // ---------------------------------------------------------------------
    // 3. get_status: query blob metadata/status. Implemented by provider.
    // ---------------------------------------------------------------------
    async fn get_status(&self, blob: &BlobId) -> Result<BlobStatus, StorageError>;

    // ---------------------------------------------------------------------
    // 4. download: return an AsyncRead (stream) for the blob. Implemented by provider.
    // ---------------------------------------------------------------------
    async fn download_blob(
        &self,
        blob: &BlobId
    ) -> Result<Box<dyn AsyncRead + Send + Unpin>, StorageError>;

    // ---------------------------------------------------------------------
    // 5. write_file: write provided AsyncRead into the given path (streaming copy).
    //    Default implementation uses tokio::io::copy to avoid loading whole file into memory.
    // ---------------------------------------------------------------------
    async fn write_file<P: AsRef<Path> + Send + Sync>(
        &self,
        path: P,
        mut reader: Box<dyn AsyncRead + Send + Unpin>
    ) -> Result<(), StorageError> {
        // Create destination file
        let mut dest = fs::File::create(path).await?;
        // tokio::io::copy requires &mut types that implement AsyncRead/AsyncWrite
        // `reader` is boxed dyn AsyncRead; use mutable reference
        let bytes_copied = io
            ::copy(&mut reader, &mut dest).await
            .map_err(|e| { StorageError::Other(format!("io copy failed: {}", e)) })?;
        // Optionally flush (File::sync_data if needed)
        // dest.sync_data().await.map_err(|e| StorageError::Other(format!("sync failed: {}", e)))?;
        let _ = bytes_copied;
        Ok(())
    }

    // ---------------------------------------------------------------------
    // 6. upload_file: convenience helper = load_file + read into memory + upload_blob
    //    Note: we buffer file content into memory here because upload_blob takes Bytes.
    //    If provider supports streaming upload you'd implement upload_blob differently.
    // ---------------------------------------------------------------------
    async fn upload_file<P: AsRef<Path> + Send + Sync>(
        &self,
        path: P,
        extra: Option<&str>
    ) -> Result<BlobId, StorageError> {
        // Default: get a reader and read_to_end into Vec<u8>
        let mut reader = self.load_file(&path).await?;
        let mut buf = Vec::new();
        reader
            .read_to_end(&mut buf).await
            .map_err(|e| {
                StorageError::Other(format!("failed to read file into buffer: {}", e))
            })?;
        self.upload_blob(Bytes::from(buf), extra).await
    }

    // ---------------------------------------------------------------------
    // 7. download_file: convenience helper = download (reader) + write_file
    // ---------------------------------------------------------------------
    async fn download_file<P: AsRef<Path> + Send + Sync>(
        &self,
        blob: &BlobId,
        out_path: P
    ) -> Result<(), StorageError> {
        let reader = self.download_blob(blob).await?;
        self.write_file(out_path, reader).await
    }
}
