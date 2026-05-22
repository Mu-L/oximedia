//! Tests for the CDN uploaders — S3, GCS, Azure.
//!
//! The CDN uploaders route through the real `oximedia-storage` backends when
//! the matching `cdn-aws` / `cdn-gcs` / `cdn-azure` feature is enabled, and
//! fall back to a pure-Rust log-only path otherwise.
//!
//! This file therefore has two test groups:
//!
//! * **Log-only tests** — compiled only when NO `cdn-*` feature is active.
//!   They run fully offline (no network, no credentials) and exercise key
//!   validation, single-part / multipart logging, and synthesised URLs.
//! * **Live integration tests** — compiled only when the matching `cdn-*`
//!   feature is active.  They are marked `#[ignore]` because they require real
//!   cloud credentials and network access; run them explicitly with
//!   `cargo test --features cdn-all -- --ignored`.

use oximedia_server::cdn::CdnConfig;

/// Build an S3/GCS-style CDN config (bucket + region + credentials).
fn default_config() -> CdnConfig {
    CdnConfig {
        backend: oximedia_server::cdn::CdnBackend::S3,
        bucket: "test-bucket".to_string(),
        region: "us-east-1".to_string(),
        access_key: "key".to_string(),
        secret_key: "secret".to_string(),
        base_path: "media".to_string(),
        public: true,
        enable_cdn: false,
        cdn_domain: None,
        project_id: Some("test-project".to_string()),
    }
}

/// Build an Azure-style CDN config: `bucket` is the container, `region` is the
/// storage account name, `secret_key` is the account access key.
fn azure_config() -> CdnConfig {
    CdnConfig {
        backend: oximedia_server::cdn::CdnBackend::Azure,
        bucket: "mycontainer".to_string(),
        region: "myaccount".to_string(),
        access_key: String::new(),
        secret_key: "YWNjb3VudGtleQ==".to_string(),
        base_path: "assets".to_string(),
        public: true,
        enable_cdn: false,
        cdn_domain: None,
        project_id: None,
    }
}

// ════════════════════════════════════════════════════════════════════════════
// CdnConfig — always compiled (no feature dependency)
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_cdn_config_default_has_no_project_id() {
    let cfg = CdnConfig::default();
    assert!(
        cfg.project_id.is_none(),
        "project_id should default to None"
    );
}

#[test]
fn test_cdn_config_project_id_is_settable() {
    let mut cfg = CdnConfig::default();
    cfg.project_id = Some("my-gcp-project".to_string());
    assert_eq!(cfg.project_id.as_deref(), Some("my-gcp-project"));
}

#[test]
fn test_cdn_config_clone_preserves_project_id() {
    let cfg = default_config();
    let cloned = cfg.clone();
    assert_eq!(cloned.project_id, cfg.project_id);
}

// ════════════════════════════════════════════════════════════════════════════
// Log-only path — compiled only when NO cdn-* feature is enabled
// ════════════════════════════════════════════════════════════════════════════

#[cfg(not(any(feature = "cdn-aws", feature = "cdn-gcs", feature = "cdn-azure")))]
mod log_only {
    use super::{azure_config, default_config};
    use oximedia_server::cdn::{AzureCdnUploader, GcsCdnUploader, S3CdnUploader};

    // ── S3CdnUploader ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_s3_upload_bytes_single_part() {
        let uploader = S3CdnUploader::new(&default_config())
            .await
            .expect("s3 init");
        let data = vec![0u8; 1024]; // 1 KiB — well below 5 MiB threshold
        let url = uploader
            .upload_bytes(&data, "clips/sample.mp4")
            .await
            .expect("upload");
        assert!(
            url.contains("test-bucket"),
            "URL should contain bucket name"
        );
        assert!(url.contains("clips/sample.mp4"), "URL should contain key");
    }

    #[tokio::test]
    async fn test_s3_upload_bytes_multipart() {
        let uploader = S3CdnUploader::new(&default_config())
            .await
            .expect("s3 init");
        let data = vec![1u8; 6 * 1024 * 1024]; // 6 MiB — triggers multipart logging
        let url = uploader
            .upload_bytes(&data, "large/file.mp4")
            .await
            .expect("multipart upload");
        assert!(!url.is_empty());
    }

    #[tokio::test]
    async fn test_s3_upload_bytes_empty_key_returns_error() {
        let uploader = S3CdnUploader::new(&default_config())
            .await
            .expect("s3 init");
        let result = uploader.upload_bytes(b"data", "").await;
        assert!(result.is_err(), "empty key should return InvalidKey error");
    }

    #[tokio::test]
    async fn test_s3_upload_bytes_path_traversal_rejected() {
        let uploader = S3CdnUploader::new(&default_config())
            .await
            .expect("s3 init");
        let result = uploader.upload_bytes(b"data", "../escape.mp4").await;
        assert!(result.is_err(), "path traversal key should be rejected");
    }

    #[tokio::test]
    async fn test_s3_upload_file() {
        let uploader = S3CdnUploader::new(&default_config())
            .await
            .expect("s3 init");
        let dir = std::env::temp_dir().join(format!("s3_test_{}", nanos()));
        tokio::fs::create_dir_all(&dir).await.expect("mkdir");
        let path = dir.join("test.mp4");
        tokio::fs::write(&path, b"fake media").await.expect("write");

        let url = uploader
            .upload(&path, "uploads/test.mp4")
            .await
            .expect("upload file");
        assert!(url.contains("uploads/test.mp4"));
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn test_s3_presigned_url() {
        let uploader = S3CdnUploader::new(&default_config()).await.expect("init");
        let url = uploader
            .presigned_url("asset.mp4", 3600)
            .await
            .expect("presigned");
        assert!(!url.is_empty());
    }

    #[tokio::test]
    async fn test_s3_delete() {
        let uploader = S3CdnUploader::new(&default_config()).await.expect("init");
        uploader.delete("asset.mp4").await.expect("delete");
    }

    #[tokio::test]
    async fn test_s3_list_returns_empty_without_feature() {
        let uploader = S3CdnUploader::new(&default_config()).await.expect("init");
        let keys = uploader.list("clips/").await.expect("list");
        assert!(keys.is_empty(), "log-only list returns empty");
    }

    // ── GcsCdnUploader ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_gcs_upload_bytes_returns_url() {
        let uploader = GcsCdnUploader::new(&default_config())
            .await
            .expect("gcs init");
        let url = uploader
            .upload_bytes(b"media data", "footage/raw.mp4")
            .await
            .expect("gcs upload bytes");
        assert!(url.contains("footage/raw.mp4"), "URL should contain key");
        assert!(
            url.contains("storage.googleapis.com"),
            "URL should use GCS domain"
        );
    }

    #[tokio::test]
    async fn test_gcs_upload_bytes_empty_key_returns_error() {
        let uploader = GcsCdnUploader::new(&default_config())
            .await
            .expect("gcs init");
        let result = uploader.upload_bytes(b"data", "").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_gcs_upload_file() {
        let uploader = GcsCdnUploader::new(&default_config())
            .await
            .expect("gcs init");
        let dir = std::env::temp_dir().join(format!("gcs_test_{}", nanos()));
        tokio::fs::create_dir_all(&dir).await.expect("mkdir");
        let path = dir.join("video.mp4");
        tokio::fs::write(&path, b"gcs video").await.expect("write");

        let url = uploader
            .upload(&path, "gcs/video.mp4")
            .await
            .expect("upload");
        assert!(!url.is_empty());
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn test_gcs_signed_url() {
        let uploader = GcsCdnUploader::new(&default_config()).await.expect("init");
        let url = uploader.signed_url("asset.mp4", 600).await.expect("signed");
        assert!(!url.is_empty());
    }

    #[tokio::test]
    async fn test_gcs_delete() {
        let uploader = GcsCdnUploader::new(&default_config()).await.expect("init");
        uploader.delete("asset.mp4").await.expect("delete");
    }

    #[tokio::test]
    async fn test_gcs_list_returns_empty_without_feature() {
        let uploader = GcsCdnUploader::new(&default_config()).await.expect("init");
        let keys = uploader.list("footage/").await.expect("list");
        assert!(keys.is_empty(), "log-only list returns empty");
    }

    // ── AzureCdnUploader ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_azure_upload_bytes_single_block() {
        let uploader = AzureCdnUploader::new(&azure_config())
            .await
            .expect("azure init");
        let url = uploader
            .upload_bytes(b"small blob", "blobs/small.mp4")
            .await
            .expect("azure upload");
        assert!(url.contains("myaccount"), "URL should contain account");
        assert!(url.contains("blobs/small.mp4"), "URL should contain key");
    }

    #[tokio::test]
    async fn test_azure_upload_bytes_multi_block() {
        let uploader = AzureCdnUploader::new(&azure_config())
            .await
            .expect("azure init");
        let data = vec![0u8; 12 * 1024 * 1024]; // 12 MiB → 3 blocks of 5 MiB + remainder
        let url = uploader
            .upload_bytes(&data, "blobs/large.mp4")
            .await
            .expect("azure multiblock upload");
        assert!(!url.is_empty());
    }

    #[tokio::test]
    async fn test_azure_upload_bytes_empty_key_returns_error() {
        let uploader = AzureCdnUploader::new(&azure_config())
            .await
            .expect("azure init");
        let result = uploader.upload_bytes(b"data", "").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_azure_upload_file() {
        let uploader = AzureCdnUploader::new(&azure_config())
            .await
            .expect("azure init");
        let dir = std::env::temp_dir().join(format!("azure_test_{}", nanos()));
        tokio::fs::create_dir_all(&dir).await.expect("mkdir");
        let path = dir.join("content.mp4");
        tokio::fs::write(&path, b"azure blob content")
            .await
            .expect("write");

        let url = uploader
            .upload(&path, "azure/content.mp4")
            .await
            .expect("azure upload file");
        assert!(!url.is_empty());
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn test_azure_sas_url() {
        let uploader = AzureCdnUploader::new(&azure_config()).await.expect("init");
        let url = uploader.sas_url("asset.mp4", 3600).await.expect("sas url");
        assert!(url.contains("blob.core.windows.net"));
    }

    #[tokio::test]
    async fn test_azure_delete() {
        let uploader = AzureCdnUploader::new(&azure_config()).await.expect("init");
        uploader.delete("blob.mp4").await.expect("delete");
    }

    #[tokio::test]
    async fn test_azure_list_returns_empty_without_feature() {
        let uploader = AzureCdnUploader::new(&azure_config()).await.expect("init");
        let keys = uploader.list("blobs/").await.expect("list");
        assert!(keys.is_empty(), "log-only list returns empty");
    }

    fn nanos() -> u32 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Live integration tests — compiled only with the matching cdn-* feature.
// Marked #[ignore]: they need real cloud credentials + network access.
// Run with e.g. `cargo test -p oximedia-server --features cdn-all -- --ignored`.
// ════════════════════════════════════════════════════════════════════════════

#[cfg(feature = "cdn-aws")]
#[tokio::test]
#[ignore = "requires real AWS S3 credentials and network access"]
async fn test_s3_live_upload_delete_roundtrip() {
    use oximedia_server::cdn::S3CdnUploader;
    let uploader = S3CdnUploader::new(&default_config())
        .await
        .expect("s3 init");
    let key = "ci-integration/roundtrip.bin";
    let payload = b"oximedia s3 integration payload".to_vec();
    let url = uploader
        .upload_bytes(&payload, key)
        .await
        .expect("s3 live upload");
    assert!(url.contains("test-bucket"));

    let listed = uploader
        .list("ci-integration/")
        .await
        .expect("s3 live list");
    assert!(
        listed.iter().any(|k| k.ends_with("roundtrip.bin")),
        "uploaded object should appear in the listing"
    );

    uploader.delete(key).await.expect("s3 live delete");
}

#[cfg(feature = "cdn-aws")]
#[tokio::test]
#[ignore = "requires real AWS S3 credentials and network access"]
async fn test_s3_live_upload_file() {
    use oximedia_server::cdn::S3CdnUploader;
    let uploader = S3CdnUploader::new(&default_config())
        .await
        .expect("s3 init");
    let dir = std::env::temp_dir().join("oximedia_s3_live");
    tokio::fs::create_dir_all(&dir).await.expect("mkdir");
    let path = dir.join("upload.bin");
    tokio::fs::write(&path, b"file payload")
        .await
        .expect("write");

    let url = uploader
        .upload(&path, "ci-integration/file-upload.bin")
        .await
        .expect("s3 live file upload");
    assert!(!url.is_empty());

    uploader
        .delete("ci-integration/file-upload.bin")
        .await
        .expect("s3 live delete");
    let _ = tokio::fs::remove_dir_all(&dir).await;
}

#[cfg(feature = "cdn-gcs")]
#[tokio::test]
#[ignore = "requires real GCS credentials and network access"]
async fn test_gcs_live_upload_delete_roundtrip() {
    use oximedia_server::cdn::GcsCdnUploader;
    let uploader = GcsCdnUploader::new(&default_config())
        .await
        .expect("gcs init");
    let key = "ci-integration/roundtrip.bin";
    let payload = b"oximedia gcs integration payload".to_vec();
    let url = uploader
        .upload_bytes(&payload, key)
        .await
        .expect("gcs live upload");
    assert!(url.contains("storage.googleapis.com"));

    let listed = uploader
        .list("ci-integration/")
        .await
        .expect("gcs live list");
    assert!(
        listed.iter().any(|k| k.ends_with("roundtrip.bin")),
        "uploaded object should appear in the listing"
    );

    uploader.delete(key).await.expect("gcs live delete");
}

#[cfg(feature = "cdn-azure")]
#[tokio::test]
#[ignore = "requires real Azure Blob Storage credentials and network access"]
async fn test_azure_live_upload_delete_roundtrip() {
    use oximedia_server::cdn::AzureCdnUploader;
    let uploader = AzureCdnUploader::new(&azure_config())
        .await
        .expect("azure init");
    let key = "ci-integration/roundtrip.bin";
    let payload = b"oximedia azure integration payload".to_vec();
    let url = uploader
        .upload_bytes(&payload, key)
        .await
        .expect("azure live upload");
    assert!(url.contains("blob.core.windows.net"));

    let listed = uploader
        .list("ci-integration/")
        .await
        .expect("azure live list");
    assert!(
        listed.iter().any(|k| k.ends_with("roundtrip.bin")),
        "uploaded object should appear in the listing"
    );

    uploader.delete(key).await.expect("azure live delete");
}
