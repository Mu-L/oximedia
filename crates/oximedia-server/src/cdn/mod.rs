//! CDN integration for uploading segments to cloud storage.

mod azure;
mod gcs;
mod s3;
mod uploader;

pub use azure::{AzureCdnUploader, AzureUploader};
pub use gcs::{GcsCdnUploader, GcsUploader};
pub use s3::{
    partition_into_parts, upload_to_s3, CdnError, MultipartConfig, S3CdnUploader, S3Uploader,
};
pub use uploader::{CdnBackend, CdnConfig, CdnUploader, UploadJob};
