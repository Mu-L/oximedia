//! CDN integration for uploading segments to cloud storage.

mod azure;
mod gcs;
mod s3;
mod uploader;

pub use azure::AzureUploader;
pub use gcs::GcsUploader;
pub use s3::S3Uploader;
pub use uploader::{CdnBackend, CdnConfig, CdnUploader, UploadJob};
