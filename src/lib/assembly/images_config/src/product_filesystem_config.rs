// Copyright 2023 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The product options for configuring the filesystem.
/// The options include which filesystems to build and how, but do not contain constraints derived
/// from the board or partition size.
#[derive(Serialize, Deserialize, Debug, Default, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductFilesystemConfig {
    /// The filename to use for the zbi and vbmeta.
    #[serde(default)]
    pub image_name: ImageName,

    /// Make fshost watch for NAND devices.
    #[serde(default)]
    pub watch_for_nand: bool,

    /// If format_data_on_corruption is true (the default), fshost formats
    /// minfs partition on finding it corrupted.  Set to false to keep the
    /// devices in a corrupted state which might be of help to debug issues.
    #[serde(default)]
    pub format_data_on_corruption: FormatDataOnCorruption,

    /// Disable zxcrypt. This argument only applies when using minfs.
    #[serde(default)]
    pub no_zxcrypt: bool,

    /// Whether the filesystem image should be placed in a separate partition,
    /// in a ramdisk, or nonexistent.
    #[serde(default)]
    pub image_mode: FilesystemImageMode,

    /// Which volume to build to hold the filesystems.
    #[serde(default)]
    pub volume: VolumeConfig,

    /// The compression algorithm that blobfs should use for writing blobs at runtime.
    /// If unset, an internally defined system default is used.
    pub blobfs_write_compression_algorithm: Option<BlobfsWriteCompressionAlgorithm>,

    /// Controls blobfs' eviction strategy for pager-backed blobs with no open
    /// handles or VMO clones. If unset, an internally defined system default is
    /// used.
    pub blobfs_cache_eviction_policy: Option<BlobfsCacheEvictionPolicy>,
}

/// The filename to use for the zbi and vbmeta.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImageName(pub String);
impl Default for ImageName {
    fn default() -> Self {
        Self("fuchsia".to_string())
    }
}

/// Whether for format the data filesystem when a corruption is detected.
#[derive(Serialize, Deserialize, Debug, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FormatDataOnCorruption(pub bool);
impl Default for FormatDataOnCorruption {
    fn default() -> Self {
        Self(true)
    }
}

/// Whether the filesystem should be placed in a separate partition, in a
/// ramdisk, or nonexistent.
#[derive(Serialize, Deserialize, Debug, Default, PartialEq, JsonSchema, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub enum FilesystemImageMode {
    /// No filesystem image should be generated.
    #[serde(rename = "no_image")]
    NoImage,

    /// The filesystem image should be placed in a ramdisk in the ZBI.
    /// TODO(awolter): Delete this once all clients pass this via the CLI.
    #[serde(rename = "ramdisk")]
    Ramdisk,

    /// The filesystem image should be placed in a separate partition.
    #[serde(rename = "partition")]
    #[default]
    Partition,
}

/// How to configure the filesystem volume.
/// Some systems may configure this without actually generating filesystem
/// images in order to configure fshost without needing an actual filesystem.
#[derive(Serialize, Deserialize, Debug, Default, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub enum VolumeConfig {
    /// A fxfs volume.
    #[serde(rename = "fxfs")]
    #[default]
    Fxfs,
    /// A fvm volume.
    #[serde(rename = "fvm")]
    Fvm(FvmVolumeConfig),
}

/// A FVM volume.
#[derive(Serialize, Deserialize, Debug, Default, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FvmVolumeConfig {
    /// Configures the data filesystem which will be used for this product.
    #[serde(default)]
    pub data: DataFvmVolumeConfig,

    /// Configures the blob filesystem which is built for this product.
    #[serde(default)]
    pub blob: BlobFvmVolumeConfig,

    /// If specified, bytes will be reserved in the fvm for this product.
    #[serde(default)]
    pub reserved: Option<ReservedFvmVolumeConfig>,
}

/// Configuration options for a data filesystem.
#[derive(Serialize, Deserialize, Debug, Default, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DataFvmVolumeConfig {
    /// If true, will enable content-detection for partition format, supporting
    /// both minfs and fxfs filesystems. A special "fs_switch" file can be
    /// written to the root directory containing the string "minfs", "fxfs" or
    /// "toggle" to trigger a migration from the current format to the specified
    /// format. (The "toggle" option will migrate back and forth at each boot.)
    #[serde(default)]
    pub use_disk_based_minfs_migration: bool,

    /// Set to one of "minfs", "fxfs", "f2fs" (unstable).
    /// If set to anything other than "minfs", any existing minfs partition will be
    /// migrated in-place to the specified format when fshost mounts it.
    /// Set by products
    #[serde(default)]
    pub data_filesystem_format: DataFilesystemFormat,
}

/// The data format to use inside the fvm.
#[derive(Serialize, Deserialize, Debug, Default, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "lowercase")]
pub enum DataFilesystemFormat {
    /// A fxfs filesystem for persisting data.
    #[default]
    Fxfs,

    /// A f2fs filesystem for persisting data.
    F2fs,

    /// A minfs filesystem for persisting data.
    Minfs,
}

/// Configuration options for a blob filesystem.
#[derive(Serialize, Deserialize, Debug, Default, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BlobFvmVolumeConfig {
    /// The format blobfs should store blobs in.
    #[serde(default)]
    pub blob_layout: BlobfsLayout,
}

/// The internal layout of blobfs.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub enum BlobfsLayout {
    /// A more compact layout than DeprecatedPadded.
    #[serde(rename = "compact")]
    #[default]
    Compact,

    /// A layout that is deprecated, but kept for compatibility reasons.
    #[serde(rename = "deprecated_padded")]
    DeprecatedPadded,
}

/// Configuration options for reserving space in the fvm.
#[derive(Serialize, Deserialize, Debug, Default, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReservedFvmVolumeConfig {
    /// The number of slices to reserve in the fvm.
    pub reserved_slices: u64,
}

/// Compression algorithms
#[derive(Serialize, Deserialize, Debug, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub enum BlobfsWriteCompressionAlgorithm {
    /// Use a ZSTD based compression scheme.
    #[serde(rename = "zstd_chunked")]
    ZSTDChunked,

    /// Do no apply compression to blobs when writing the to disk.
    #[serde(rename = "uncompressed")]
    Uncompressed,
}

/// Eviction policies for blob cache management.
#[derive(Serialize, Deserialize, Debug, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub enum BlobfsCacheEvictionPolicy {
    /// Blobs that are not pager-backed are not affected by this knob.
    /// Nodes are never evicted. It is recommended to enable kernel
    /// page eviction (`kernel.page-scanner.enable-eviction`) in this case, as
    /// otherwise blobfs will indefinitely retain all data pages in memory.
    #[serde(rename = "never_evict")]
    NeverEvict,

    /// Nodes are evicted as soon as they have no open handles or VMO clones.
    /// They will need to be loaded from disk again on next access.
    #[serde(rename = "evict_immediately")]
    EvictImmediately,
}
