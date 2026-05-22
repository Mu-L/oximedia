//! CDN integration for uploading segments to cloud storage.

mod azure;
mod gcs;
mod s3;
mod uploader;

pub use azure::{AzureCdnUploader, AzureUploader};
pub use gcs::{GcsCdnUploader, GcsUploader};
pub use s3::{CdnError, S3CdnUploader, S3Uploader};
pub use uploader::{CdnBackend, CdnConfig, CdnUploader, UploadJob};
