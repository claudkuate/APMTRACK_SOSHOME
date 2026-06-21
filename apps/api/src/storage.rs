use anyhow::{anyhow, Result};
use s3::creds::Credentials;
use s3::region::Region;
use s3::Bucket;

use crate::config::S3Config;

/// Taille maximale d'un avatar (agent / utilisateur) accepte a l'upload.
pub const MAX_AVATAR_BYTES: usize = 5 * 1024 * 1024;

/// Extension de fichier deduite d'un content-type image (repli `jpg`).
pub fn image_extension(content_type: &str) -> &'static str {
    match content_type {
        "image/png" => "png",
        "image/webp" => "webp",
        "image/heic" | "image/heif" => "heic",
        "image/gif" => "gif",
        _ => "jpg",
    }
}

/// Content-type deduit d'une cle objet (par extension), pour servir un avatar.
pub fn content_type_for_key(key: &str) -> &'static str {
    match key.rsplit('.').next() {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("heic") | Some("heif") => "image/heic",
        Some("gif") => "image/gif",
        _ => "image/jpeg",
    }
}

/// Thin wrapper around an S3/MinIO bucket used to store PV photos.
///
/// Uploads and downloads are proxied through the API (the bucket stays on the
/// internal network), so the mobile client never talks to MinIO directly.
pub struct ObjectStorage {
    bucket: Box<Bucket>,
}

impl ObjectStorage {
    pub fn from_config(config: &S3Config) -> Result<Self> {
        let region = Region::Custom {
            region: config.region.clone(),
            endpoint: config.endpoint.clone(),
        };
        let credentials = Credentials::new(
            Some(&config.access_key),
            Some(&config.secret_key),
            None,
            None,
            None,
        )?;
        // Path-style addressing is required for MinIO (no virtual-hosted buckets).
        let bucket = Bucket::new(&config.bucket, region, credentials)?.with_path_style();
        Ok(Self { bucket })
    }

    pub async fn put(&self, key: &str, bytes: &[u8], content_type: &str) -> Result<()> {
        let response = self
            .bucket
            .put_object_with_content_type(key, bytes, content_type)
            .await?;
        let status = response.status_code();
        if !(200..300).contains(&status) {
            return Err(anyhow!("object storage put failed with status {status}"));
        }
        Ok(())
    }

    pub async fn get(&self, key: &str) -> Result<Vec<u8>> {
        let response = self.bucket.get_object(key).await?;
        let status = response.status_code();
        if !(200..300).contains(&status) {
            return Err(anyhow!("object storage get failed with status {status}"));
        }
        Ok(response.bytes().to_vec())
    }

    pub async fn delete(&self, key: &str) -> Result<()> {
        self.bucket.delete_object(key).await?;
        Ok(())
    }
}
