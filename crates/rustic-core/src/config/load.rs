use std::{io::Write, path::PathBuf};

use anyhow::Result;
use google_cloud_storage::{
    client::{Client, ClientConfig},
    http::objects::{download::Range, get::GetObjectRequest},
};
use tempfile::NamedTempFile;

/// Loads raw bytes from a local file path or a GCS path (`gs://bucket/object`).
pub async fn load_bytes(path: String) -> Result<Vec<u8>> {
    let bytes = if path.starts_with("gs://") {
        download_gcs_bytes(&path).await?
    } else {
        std::fs::read(&path)?
    };
    Ok(bytes)
}

/// Loads UTF-8 text content from a local file path or a GCS path (`gs://bucket/object`).
pub async fn load_content(path: String) -> Result<String> {
    let content = if path.starts_with("gs://") {
        download_gcs_string(&path).await?
    } else {
        std::fs::read_to_string(&path)?
    };
    Ok(content)
}

/// Downloads raw bytes from a GCS object.
pub async fn download_gcs_bytes(gcs_path: &str) -> anyhow::Result<Vec<u8>> {
    let path = gcs_path
        .strip_prefix("gs://")
        .ok_or_else(|| anyhow::anyhow!("Invalid GCS path: {}", gcs_path))?;
    let (bucket, object) = path
        .split_once('/')
        .ok_or_else(|| anyhow::anyhow!("Invalid GCS path: {}", gcs_path))?;

    let config = ClientConfig::default().with_auth().await?;
    let client = Client::new(config);

    let data = client
        .download_object(
            &GetObjectRequest {
                bucket: bucket.to_string(),
                object: object.to_string(),
                ..Default::default()
            },
            &Range::default(),
        )
        .await?;

    Ok(data)
}

/// Downloads a GCS object to a temp file and returns its path (e.g. for xlsx, model files).
/// The caller owns the file and must delete it when done.
pub async fn download_gcs_to_file(gcs_path: &str) -> anyhow::Result<PathBuf> {
    let data = download_gcs_bytes(gcs_path).await?;
    let mut tmp = NamedTempFile::new()?;
    tmp.write_all(&data)?;
    Ok(tmp.into_temp_path().keep()?)
}

/// Downloads a GCS object and decodes it as UTF-8 text (e.g. for config, JSON).
pub async fn download_gcs_string(gcs_path: &str) -> anyhow::Result<String> {
    let data = download_gcs_bytes(gcs_path).await?;
    Ok(String::from_utf8(data)?)
}
