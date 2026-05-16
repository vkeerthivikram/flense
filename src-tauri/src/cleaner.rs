// src-tauri/src/cleaner.rs
// Safe cleaning with backups, atomic writes, dry-run, and native per-format cleaning.

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::errors::{AppError, AppResult};
use crate::types::{
    BackupMode, CleanConfig, CleanFileResult, CleanStatus, FileType,
    MetadataItem, RemovalCapability,
};
use crate::external_tools::{exiftool_clean, ffmpeg_clean};

// ──────────────────────────── File Hashing ───────────────────────────────────

pub fn hash_file(path: &Path) -> AppResult<String> {
    let content = fs::read(path).map_err(|e| AppError::FileRead {
        path: path.to_string_lossy().to_string(),
        source: e,
    })?;
    let mut hasher = Sha256::new();
    hasher.update(&content);
    Ok(hex::encode(hasher.finalize()))
}

// ──────────────────────────── Backup ─────────────────────────────────────────

pub fn create_backup(file_path: &Path, config: &CleanConfig) -> AppResult<PathBuf> {
    let backup_path = match &config.backup_mode {
        BackupMode::Adjacent => {
            let parent = file_path.parent().unwrap_or(Path::new(""));
            let stem = file_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");
            let mut counter = 0;
            loop {
                let backup_name = if counter == 0 {
                    format!("{}.bak", stem)
                } else {
                    format!("{}.{}.bak", stem, counter)
                };
                let candidate = parent.join(backup_name);
                if !candidate.exists() {
                    break candidate;
                }
                counter += 1;
            }
        }
        BackupMode::Directory => {
            let backup_dir = config.backup_directory.as_ref().ok_or_else(|| {
                AppError::BackupFailed("No backup directory configured".to_string())
            })?;
            let backup_dir_path = PathBuf::from(backup_dir);
            if !backup_dir_path.exists() {
                fs::create_dir_all(&backup_dir_path).map_err(|e| {
                    AppError::BackupFailed(format!("Failed to create backup directory: {}", e))
                })?;
            }
            let stem = file_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");
            let ext = file_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            let mut counter = 0;
            loop {
                let backup_name = if counter == 0 {
                    format!("{}_{}.bak", stem, ext)
                } else {
                    format!("{}_{}_{}.bak", stem, counter, ext)
                };
                let candidate = backup_dir_path.join(backup_name);
                if !candidate.exists() {
                    break candidate;
                }
                counter += 1;
            }
        }
    };

    fs::copy(file_path, &backup_path).map_err(|e| AppError::BackupFailed(format!(
        "Failed to copy {} to {}: {}",
        file_path.display(),
        backup_path.display(),
        e
    )))?;

    Ok(backup_path)
}

// ──────────────────────────── Cleaning Operations ────────────────────────────

pub fn clean_file(
    file_path: &Path,
    config: &CleanConfig,
    items_to_remove: &[MetadataItem],
) -> AppResult<CleanFileResult> {
    validate_path(file_path)?;

    if !file_path.exists() {
        return Ok(CleanFileResult {
            file_path: file_path.to_string_lossy().to_string(),
            status: CleanStatus::Skipped,
            backup_path: None,
            metadata_removed_count: 0,
            error: Some("File not found".to_string()),
            warning: None,
        });
    }

    let hash_before = hash_file(file_path).unwrap_or_default();

    if config.dry_run {
        return Ok(CleanFileResult {
            file_path: file_path.to_string_lossy().to_string(),
            status: CleanStatus::DryRun,
            backup_path: None,
            metadata_removed_count: items_to_remove.len(),
            error: None,
            warning: Some("Dry run — no files were modified".to_string()),
        });
    }

    let removable: Vec<_> = items_to_remove
        .iter()
        .filter(|item| item.capability == RemovalCapability::Removable)
        .collect();

    if removable.is_empty() {
        return Ok(CleanFileResult {
            file_path: file_path.to_string_lossy().to_string(),
            status: CleanStatus::Skipped,
            backup_path: None,
            metadata_removed_count: 0,
            error: None,
            warning: Some("No removable metadata items selected".to_string()),
        });
    }

    let file_type = crate::metadata_scanner::detect_file_type(file_path);
    let backup_path = create_backup(file_path, config)?;

    let result = match file_type {
        FileType::Image => clean_image(file_path, &removable),
        FileType::Pdf => clean_pdf(file_path, &removable),
        FileType::Audio => clean_audio(file_path, &removable),
        FileType::Video => clean_video(file_path, &removable),
        FileType::Document => clean_document(file_path, &removable),
        FileType::Other => Err(AppError::UnsupportedFormat("Unknown".to_string())),
    };

    match result {
        Ok(()) => {
            let hash_after = hash_file(file_path).ok();
            Ok(CleanFileResult {
                file_path: file_path.to_string_lossy().to_string(),
                status: CleanStatus::Cleaned,
                backup_path: Some(backup_path.to_string_lossy().to_string()),
                metadata_removed_count: removable.len(),
                error: None,
                warning: None,
            })
        }
        Err(e) => {
            if let Err(restore_err) = fs::copy(&backup_path, file_path) {
                return Ok(CleanFileResult {
                    file_path: file_path.to_string_lossy().to_string(),
                    status: CleanStatus::Failed,
                    backup_path: Some(backup_path.to_string_lossy().to_string()),
                    metadata_removed_count: 0,
                    error: Some(format!(
                        "Cleaning failed and backup restore also failed: {} | {}",
                        e, restore_err
                    )),
                    warning: None,
                });
            }
            Ok(CleanFileResult {
                file_path: file_path.to_string_lossy().to_string(),
                status: CleanStatus::Failed,
                backup_path: Some(backup_path.to_string_lossy().to_string()),
                metadata_removed_count: 0,
                error: Some(e.to_user_message()),
                warning: Some("File has been restored from backup".to_string()),
            })
        }
    }
}

// ──────────────────── Image Cleaning ────────────────────────────

fn clean_image(file_path: &Path, _items: &[&MetadataItem]) -> AppResult<()> {
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "jpg" | "jpeg" | "tiff" | "tif" => {
            clean_image_reencode(file_path, &ext)
        }
        "png" => clean_png_metadata(file_path),
        "webp" => clean_webp_metadata(file_path),
        "bmp" => Ok(()), // BMP has no metadata to clean
        _ => {
            let temp_path = file_path.with_extension("tmp");
            exiftool_clean(file_path, &temp_path)?;
            atomic_replace(file_path, &temp_path)
        }
    }
}

/// Clean JPEG/TIFF by decoding and re-encoding without EXIF.
/// NOTE: This re-encodes and may cause minor quality loss.
fn clean_image_reencode(file_path: &Path, ext: &str) -> AppResult<()> {
    let img = image::ImageReader::open(file_path)
        .map_err(|e| AppError::FileRead {
            path: file_path.to_string_lossy().to_string(),
            source: e,
        })?
        .decode()
        .map_err(|e| AppError::CorruptedFile(format!("{}: {}", file_path.display(), e)))?;

    let temp_path = file_path.with_extension("tmp");

    if ext == "jpg" || ext == "jpeg" {
        img.save_with_format(&temp_path, image::ImageFormat::Jpeg)
            .map_err(|e| AppError::FileWrite {
                path: temp_path.to_string_lossy().to_string(),
                source: e,
            })?;
    } else {
        img.save_with_format(&temp_path, image::ImageFormat::Tiff)
            .map_err(|e| AppError::FileWrite {
                path: temp_path.to_string_lossy().to_string(),
                source: e,
            })?;
    }

    atomic_replace(file_path, &temp_path)
}

/// Clean PNG by rebuilding without tEXt/iTXt chunks.
fn clean_png_metadata(file_path: &Path) -> AppResult<()> {
    use std::fs::File;
    use std::io::BufReader;

    let file = File::open(file_path).map_err(|e| AppError::FileRead {
        path: file_path.to_string_lossy().to_string(),
        source: e,
    })?;

    let decoder = png::Decoder::new(BufReader::new(file));
    let mut reader = decoder.read_info().map_err(|e| {
        AppError::CorruptedFile(format!("PNG decode failed: {}", e))
    })?;

    let info = reader.info();
    let width = info.width;
    let height = info.height();
    let color_type = info.color_type;
    let bit_depth = info.bit_depth;

    // Read all pixel data
    let mut pixel_data = vec![0u8; reader.output_buffer_size()];
    let _ = reader.next_frame(&mut pixel_data).map_err(|e| {
        AppError::CorruptedFile(format!("PNG read failed: {}", e))
    })?;

    // Write new PNG without metadata
    let temp_path = file_path.with_extension("tmp");
    let file_out = File::create(&temp_path).map_err(|e| AppError::FileWrite {
        path: temp_path.to_string_lossy().to_string(),
        source: e,
    })?;

    let mut encoder = png::Encoder::new(file_out, width, height);
    encoder.set_color(color_type);
    encoder.set_depth(bit_depth);
    // Don't set any text chunks — this strips all metadata

    let mut writer = encoder.write_header().map_err(|e| {
        AppError::FileWrite {
            path: temp_path.to_string_lossy().to_string(),
            source: e,
        }
    })?;

    writer.write_image_data(&pixel_data).map_err(|e| {
        AppError::FileWrite {
            path: temp_path.to_string_lossy().to_string(),
            source: e,
        }
    })?;

    atomic_replace(file_path, &temp_path)
}

/// Clean WebP by removing EXIF/XMP/ICCP chunks.
fn clean_webp_metadata(file_path: &Path) -> AppResult<()> {
    let data = fs::read(file_path).map_err(|e| AppError::FileRead {
        path: file_path.to_string_lossy().to_string(),
        source: e,
    })?;

    if data.len() < 16 || &data[0..4] != b"RIFF" || &data[8..12] != b"WEBP" {
        return Err(AppError::CorruptedFile("Not a valid WebP file".to_string()));
    }

    let mut output = Vec::with_capacity(data.len());
    output.extend_from_slice(&data[0..12]); // RIFF + size + WEBP

    let mut pos = 12;
    while pos + 8 <= data.len() {
        let chunk_fourcc = &data[pos..pos + 4];
        let chunk_size = u32::from_le_bytes([
            data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7],
        ]) as usize;

        let fourcc_str = String::from_utf8_lossy(chunk_fourcc);

        // Skip EXIF, XMP, and ICCP chunks
        if fourcc_str == "EXIF" || fourcc_str == "XMP " || fourcc_str == "ICCP" {
            pos += 8 + chunk_size + (chunk_size % 2);
            continue;
        }

        // Keep this chunk
        output.extend_from_slice(&data[pos..pos + 8 + chunk_size]);
        if chunk_size % 2 == 1 {
            output.push(0); // padding byte
        }

        pos += 8 + chunk_size + (chunk_size % 2);
    }

    // Update RIFF size
    let new_size = (output.len() - 8) as u32;
    output[4..8].copy_from_slice(&new_size.to_le_bytes());

    let temp_path = file_path.with_extension("tmp");
    fs::write(&temp_path, output).map_err(|e| AppError::FileWrite {
        path: temp_path.to_string_lossy().to_string(),
        source: e,
    })?;

    atomic_replace(file_path, &temp_path)
}

// ──────────────────── PDF Cleaning ────────────────────────────

fn clean_pdf(file_path: &Path, _items: &[&MetadataItem]) -> AppResult<()> {
    let mut doc = lopdf::Document::load(file_path)
        .map_err(|e| AppError::PdfParse(format!("{}: {}", file_path.display(), e)))?;

    // Remove Info dictionary entries
    if let Ok(info_ref) = doc.trailer.get(b"Info") {
        if let lopdf::Object::Reference(info_id) = info_ref {
            if let Ok(info_dict) = doc.get_object_mut(*info_id) {
                if let lopdf::Object::Dictionary(ref mut dict) = info_dict {
                    let keys_to_remove = vec![
                        b"Author", b"Creator", b"Producer", b"CreationDate",
                        b"ModDate", b"Title", b"Subject", b"Keywords",
                    ];
                    for key in keys_to_remove {
                        let _ = dict.remove(key);
                    }
                }
            }
        }
    }

    // Remove XMP metadata if present
    if let Ok(xmp_id) = doc.get_object_id(b"Metadata") {
        let _ = doc.delete_object(xmp_id);
    }

    let temp_path = file_path.with_extension("tmp");
    doc.save(&temp_path).map_err(|e| AppError::FileWrite {
        path: temp_path.to_string_lossy().to_string(),
        source: e,
    })?;

    atomic_replace(file_path, &temp_path)
}

// ──────────────────── Audio Cleaning ────────────────────────────

fn clean_audio(file_path: &Path, _items: &[&MetadataItem]) -> AppResult<()> {
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "mp3" => {
            id3::Tag::remove_from_path(file_path, id3::Version::Id3v24)
                .map_err(|e| AppError::FileWrite {
                    path: file_path.to_string_lossy().to_string(),
                    source: e,
                })?;
            Ok(())
        }
        "flac" => clean_flac_metadata(file_path),
        "ogg" => clean_ogg_metadata(file_path),
        "m4a" | "aac" => {
            let temp_path = file_path.with_extension("tmp");
            exiftool_clean(file_path, &temp_path)?;
            atomic_replace(file_path, &temp_path)
        }
        "wav" => clean_wav_info(file_path),
        "wma" => {
            Err(AppError::UnsupportedFormat(
                "WMA cleaning requires ffmpeg. Install ffmpeg and retry.".to_string()
            ))
        }
        _ => Err(AppError::UnsupportedFormat(format!(
            "Unsupported audio format: {}", ext
        ))),
    }
}

/// Remove all Vorbis comments from a FLAC file.
fn clean_flac_metadata(file_path: &Path) -> AppResult<()> {
    use std::fs::File;
    let file = File::open(file_path).map_err(|e| AppError::FileRead {
        path: file_path.to_string_lossy().to_string(),
        source: e,
    })?;
    let mut reader = metaflac::Reader::new(file);
    let mut found = false;
    for block in reader.blocks() {
        if let Ok(metaflac::Block::VorbisComment(ref mut vc)) = block {
            vc.comments.clear();
            found = true;
            break;
        }
    }
    if found {
        let temp_path = file_path.with_extension("tmp");
        let out_file = File::create(&temp_path).map_err(|e| AppError::FileWrite {
            path: temp_path.to_string_lossy().to_string(),
            source: e,
        })?;
        let mut writer = metaflac::Writer::new(out_file)
            .map_err(|e| AppError::Unknown(format!("FLAC write init: {}", e)))?;
        let file2 = File::open(file_path).map_err(|e| AppError::FileRead {
            path: file_path.to_string_lossy().to_string(),
            source: e,
        })?;
        let mut reader2 = metaflac::Reader::new(file2);
        for block_result in reader2.blocks() {
            if let Ok(block) = block_result {
                writer.write_block(&block).map_err(|e| {
                    AppError::FileWrite {
                        path: temp_path.to_string_lossy().to_string(),
                        source: e,
                    }
                })?;
            }
        }
        writer.finish().map_err(|e| AppError::FileWrite {
            path: temp_path.to_string_lossy().to_string(),
            source: e,
        })?;
        atomic_replace(file_path, &temp_path)
    } else {
        Ok(())
    }
}

/// Remove Vorbis comments from an OGG file by rebuilding it.
fn clean_ogg_metadata(file_path: &Path) -> AppResult<()> {
    let temp_path = file_path.with_extension("tmp");
    exiftool_clean(file_path, &temp_path)?;
    atomic_replace(file_path, &temp_path)
}

/// Remove LIST INFO chunk from a WAV file.
/// WAV uses RIFF format: RIFF header + WAVE type + chunks.
/// The INFO metadata is stored in a LIST chunk with type "INFO".
fn clean_wav_info(file_path: &Path) -> AppResult<()> {
    let data = fs::read(file_path).map_err(|e| AppError::FileRead {
        path: file_path.to_string_lossy().to_string(),
        source: e,
    })?;

    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return Err(AppError::CorruptedFile("Not a valid WAV file".to_string()));
    }

    let mut output = Vec::with_capacity(data.len());
    output.extend_from_slice(&data[0..8]); // RIFF + size placeholder
    output.extend_from_slice(b"WAVE");

    let mut pos = 12;
    while pos + 8 <= data.len() {
        let chunk_id = &data[pos..pos + 4];
        let chunk_size = u32::from_le_bytes([
            data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7],
        ]) as usize;

        // Skip LIST INFO chunks entirely
        if chunk_id == b"LIST" && chunk_size >= 4 {
            let list_type = &data[pos + 8..pos + 12];
            if list_type == b"INFO" {
                pos += 8 + chunk_size + (chunk_size % 2);
                continue;
            }
        }

        // Keep all other chunks
        let chunk_end = pos + 8 + chunk_size + (chunk_size % 2);
        if chunk_end <= data.len() {
            output.extend_from_slice(&data[pos..chunk_end]);
        }
        pos = chunk_end;
    }

    // Update RIFF size
    let new_size = (output.len() - 8) as u32;
    output[4..8].copy_from_slice(&new_size.to_le_bytes());

    let temp_path = file_path.with_extension("tmp");
    fs::write(&temp_path, output).map_err(|e| AppError::FileWrite {
        path: temp_path.to_string_lossy().to_string(),
        source: e,
    })?;

    atomic_replace(file_path, &temp_path)
}

// ──────────────────── Video Cleaning ────────────────────────────

fn clean_video(file_path: &Path, _items: &[&MetadataItem]) -> AppResult<()> {
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "mp4" | "m4v" | "mov" => {
            let temp_path = file_path.with_extension("tmp");
            exiftool_clean(file_path, &temp_path)?;
            atomic_replace(file_path, &temp_path)
        }
        "avi" => clean_avi_info(file_path),
        "mkv" | "webm" => clean_mkv_tags(file_path),
        "wmv" => {
            let temp_path = file_path.with_extension("tmp");
            ffmpeg_clean(file_path, &temp_path)?;
            atomic_replace(file_path, &temp_path)
        }
        _ => Err(AppError::UnsupportedFormat(format!(
            "Unsupported video format: {}", ext
        ))),
    }
}

/// Remove LIST INFO chunk from an AVI file.
/// AVI uses RIFF format with "AVI " or "AVIX" type, metadata in LIST INFO.
fn clean_avi_info(file_path: &Path) -> AppResult<()> {
    let data = fs::read(file_path).map_err(|e| AppError::FileRead {
        path: file_path.to_string_lossy().to_string(),
        source: e,
    })?;

    if data.len() < 12 || &data[0..4] != b"RIFF" {
        return Err(AppError::CorruptedFile("Not a valid AVI file".to_string()));
    }
    if &data[8..12] != b"AVI " && &data[8..12] != b"AVIX" {
        return Err(AppError::CorruptedFile("Not a valid AVI file".to_string()));
    }

    let mut output = Vec::with_capacity(data.len());
    output.extend_from_slice(&data[0..8]); // RIFF + size placeholder
    output.extend_from_slice(&data[8..12]); // AVI or AVIX

    let mut pos = 12;
    while pos + 8 <= data.len() {
        let chunk_id = &data[pos..pos + 4];
        let chunk_size = u32::from_le_bytes([
            data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7],
        ]) as usize;

        // Skip LIST INFO chunks
        if chunk_id == b"LIST" && chunk_size >= 4 {
            let list_type = &data[pos + 8..pos + 12];
            if list_type == b"INFO" {
                pos += 8 + chunk_size + (chunk_size % 2);
                continue;
            }
        }

        // Keep all other chunks
        let chunk_end = pos + 8 + chunk_size + (chunk_size % 2);
        if chunk_end <= data.len() {
            output.extend_from_slice(&data[pos..chunk_end]);
        }
        pos = chunk_end;
    }

    // Update RIFF size
    let new_size = (output.len() - 8) as u32;
    output[4..8].copy_from_slice(&new_size.to_le_bytes());

    let temp_path = file_path.with_extension("tmp");
    fs::write(&temp_path, output).map_err(|e| AppError::FileWrite {
        path: temp_path.to_string_lossy().to_string(),
        source: e,
    })?;

    atomic_replace(file_path, &temp_path)
}

/// Remove Tags from MKV/WebM files by parsing EBML and stripping Tags elements.
/// EBML element ID for Tags: 0x1254
/// EBML element ID for Segment: 0x18538067
/// We find the Tags element within the Segment and remove it.
fn clean_mkv_tags(file_path: &Path) -> AppResult<()> {
    let data = fs::read(file_path).map_err(|e| AppError::FileRead {
        path: file_path.to_string_lossy().to_string(),
        source: e,
    })?;

    if data.len() < 4 || data[0] != 0x1A {
        return Err(AppError::CorruptedFile("Not a valid EBML file".to_string()));
    }

    // Find Segment element (ID: 0x18 0x53 0x80 0x67)
    let mut seg_pos = 0;
    let segment_id = [0x18u8, 0x53, 0x80, 0x67];
    while seg_pos + 4 < data.len() {
        if data[seg_pos..seg_pos + 4] == segment_id {
            break;
        }
        seg_pos += 1;
    }
    if seg_pos + 4 >= data.len() {
        return Err(AppError::CorruptedFile("No Segment found in MKV".to_string()));
    }

    // Parse Segment size
    let seg_size_offset = seg_pos + 4;
    let (seg_size, seg_hdr_len) = match parse_ebml_size(&data, seg_size_offset) {
        Some(v) => v,
        None => return Err(AppError::CorruptedFile("Invalid EBML size".to_string())),
    };
    let seg_data_start = seg_size_offset + seg_hdr_len;
    let seg_data_end = if seg_size == u64::MAX {
        // Unknown size — read to end of file
        data.len()
    } else {
        seg_data_start + seg_size as usize
    };

    // Find Tags element within Segment (ID: 0x12 0x54)
    let tags_id = [0x12u8, 0x54];
    let mut tags_ranges: Vec<(usize, usize)> = Vec::new();
    let mut pos = seg_data_start;

    while pos + 2 < seg_data_end && pos + 2 < data.len() {
        if data[pos] == tags_id[0] && data[pos + 1] == tags_id[1] {
            let size_offset = pos + 2;
            if let Some((elem_size, hdr_len)) = parse_ebml_size(&data, size_offset) {
                let elem_start = pos;
                let elem_end = size_offset + hdr_len + elem_size as usize;
                if elem_end <= data.len() {
                    tags_ranges.push((elem_start, elem_end));
                    pos = elem_end;
                    continue;
                }
            }
        }
        pos += 1;
    }

    if tags_ranges.is_empty() {
        return Ok(()); // No Tags to remove
    }

    // Build output without Tags elements
    let mut output = Vec::with_capacity(data.len());
    let mut copy_pos = 0;

    for (tag_start, tag_end) in &tags_ranges {
        // Copy everything before this Tags element
        if *tag_start > copy_pos {
            output.extend_from_slice(&data[copy_pos..*tag_start]);
        }
        copy_pos = *tag_end;
    }
    // Copy remainder after last Tags element
    if copy_pos < data.len() {
        output.extend_from_slice(&data[copy_pos..]);
    }

    // Update RIFF size — no, MKV doesn't use RIFF
    // Update EBML DocType element if present — not needed
    // Update Segment size to reflect new size
    let new_seg_size = (output.len() as u64) - (seg_data_start as u64);
    // Rewrite the Segment size field
    let seg_size_bytes = seg_hdr_len - 4; // size bytes only (not ID)
    if seg_data_start + seg_size_bytes <= output.len() && seg_size_bytes <= 8 {
        // This is tricky because EBML variable-size encoding
        // For simplicity, we'll leave the size as-is if it was unknown (0xFF...)
        // or rewrite if it fits
        if seg_size != u64::MAX {
            // Rewrite with same header length
            let encoded = encode_ebml_size(new_seg_size, seg_size_bytes);
            let start = seg_size_offset;
            if start + encoded.len() <= output.len() {
                for (i, &b) in encoded.iter().enumerate() {
                    output[start + i] = b;
                }
            }
        }
    }

    let temp_path = file_path.with_extension("tmp");
    fs::write(&temp_path, output).map_err(|e| AppError::FileWrite {
        path: temp_path.to_string_lossy().to_string(),
        source: e,
    })?;

    atomic_replace(file_path, &temp_path)
}

/// Parse EBML variable-size integer. Returns (value, total_bytes_consumed).
fn parse_ebml_size(data: &[u8], offset: usize) -> Option<(u64, usize)> {
    if offset >= data.len() {
        return None;
    }
    let first = data[offset];
    if first == 0 {
        return None;
    }
    let size_len = first.leading_zeros() as usize + 1;
    if offset + size_len > data.len() || size_len > 8 {
        return None;
    }
    let mask = 0xFFu8 >> size_len;
    let mut value = (first & mask) as u64;
    for i in 1..size_len {
        value = (value << 8) | data[offset + i] as u64;
    }
    // Check for unknown size (all 1s after the leading 1)
    if size_len == 8 && value == u64::MAX {
        return Some((u64::MAX, size_len));
    }
    Some((value, size_len))
}

/// Encode a value as an EBML variable-size integer with a specific byte length.
fn encode_ebml_size(value: u64, byte_len: usize) -> Vec<u8> {
    let mut result = vec![0u8; byte_len];
    let mut v = value;
    for i in (0..byte_len).rev() {
        result[i] = (v & 0xFF) as u8;
        v >>= 8;
    }
    // Set the leading bit of the first byte
    result[0] |= 1u8 << (8 - byte_len);
    result
}

// ──────────────────── Document Cleaning ────────────────────────────

fn clean_document(file_path: &Path, _items: &[&MetadataItem]) -> AppResult<()> {
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "docx" | "xlsx" | "pptx" => clean_office_metadata(file_path),
        "odt" | "ods" | "odp" => clean_odf_metadata(file_path),
        _ => Err(AppError::UnsupportedFormat(format!(
            "{} document cleaning not supported", ext.to_uppercase()
        ))),
    }
}

/// Clean Office Open XML metadata by replacing docProps/core.xml.
fn clean_office_metadata(file_path: &Path) -> AppResult<()> {
    use std::collections::HashMap;
    use std::fs::File;
    use std::io::{BufReader, Cursor};
    use zip::write::SimpleFileOptions;

    let temp_path = file_path.with_extension("tmp");

    // Read all entries from the original ZIP
    let file = File::open(file_path).map_err(|e| AppError::FileRead {
        path: file_path.to_string_lossy().to_string(),
        source: e,
    })?;

    let mut archive = zip::ZipArchive::new(file).map_err(|e| {
        AppError::Unknown(format!("ZIP open error: {}", e))
    })?;

    // Collect all entries (name -> content)
    let mut entries: HashMap<String, Vec<u8>> = HashMap::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| {
            AppError::Unknown(format!("ZIP entry read error: {}", e))
        })?;

        let name = entry.name().to_string();
        let mut content = Vec::new();
        entry.read_to_end(&mut content).map_err(|e| {
            AppError::Unknown(format!("ZIP content read error: {}", e))
        })?;
        entries.insert(name, content);
    }

    // Replace or create docProps/core.xml with empty properties
    let empty_core = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
</cp:coreProperties>"#;

    entries.insert("docProps/core.xml".to_string(), empty_core.as_bytes().to_vec());

    // Write new ZIP
    let out_file = File::create(&temp_path).map_err(|e| AppError::FileWrite {
        path: temp_path.to_string_lossy().to_string(),
        source: e,
    })?;

    let mut writer = zip::ZipWriter::new(out_file);

    // Sort entries for deterministic output
    let mut sorted_entries: Vec<_> = entries.iter().collect();
    sorted_entries.sort_by(|a, b| a.0.cmp(b.0));

    for (name, content) in sorted_entries {
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);

        writer.start_file(name, options).map_err(|e| {
            AppError::FileWrite {
                path: temp_path.to_string_lossy().to_string(),
                source: e,
            }
        })?;

        writer.write_all(content).map_err(|e| {
            AppError::FileWrite {
                path: temp_path.to_string_lossy().to_string(),
                source: e,
            }
        })?;
    }

    writer.finish().map_err(|e| AppError::FileWrite {
        path: temp_path.to_string_lossy().to_string(),
        source: e,
    })?;

    atomic_replace(file_path, &temp_path)
}

/// Clean ODF metadata by replacing meta.xml.
fn clean_odf_metadata(file_path: &Path) -> AppResult<()> {
    use std::collections::HashMap;
    use std::fs::File;
    use std::io::Read;
    use zip::write::SimpleFileOptions;

    let temp_path = file_path.with_extension("tmp");

    let file = File::open(file_path).map_err(|e| AppError::FileRead {
        path: file_path.to_string_lossy().to_string(),
        source: e,
    })?;

    let mut archive = zip::ZipArchive::new(file).map_err(|e| {
        AppError::Unknown(format!("ZIP open error: {}", e))
    })?;

    let mut entries: HashMap<String, Vec<u8>> = HashMap::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| {
            AppError::Unknown(format!("ZIP entry read error: {}", e))
        })?;
        let name = entry.name().to_string();
        let mut content = Vec::new();
        entry.read_to_end(&mut content).map_err(|e| {
            AppError::Unknown(format!("ZIP content read error: {}", e))
        })?;
        entries.insert(name, content);
    }

    // Replace meta.xml with clean version
    let clean_meta = r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-meta xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:xlink="http://www.w3.org/1999/xlink" office:version="1.3">
  <office:meta>
  </office:meta>
</office:document-meta>"#;

    entries.insert("meta.xml".to_string(), clean_meta.as_bytes().to_vec());

    let out_file = File::create(&temp_path).map_err(|e| AppError::FileWrite {
        path: temp_path.to_string_lossy().to_string(),
        source: e,
    })?;

    let mut writer = zip::ZipWriter::new(out_file);
    let mut sorted_entries: Vec<_> = entries.iter().collect();
    sorted_entries.sort_by(|a, b| a.0.cmp(b.0));

    for (name, content) in sorted_entries {
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);
        writer.start_file(name, options).map_err(|e| {
            AppError::FileWrite {
                path: temp_path.to_string_lossy().to_string(),
                source: e,
            }
        })?;
        writer.write_all(content).map_err(|e| {
            AppError::FileWrite {
                path: temp_path.to_string_lossy().to_string(),
                source: e,
            }
        })?;
    }

    writer.finish().map_err(|e| AppError::FileWrite {
        path: temp_path.to_string_lossy().to_string(),
        source: e,
    })?;

    atomic_replace(file_path, &temp_path)
}

// ──────────────────────────── Atomic Write ───────────────────────────────────

fn atomic_replace(target: &Path, source: &Path) -> AppResult<()> {
    if let Ok(metadata) = fs::metadata(target) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = metadata.permissions();
            let _ = fs::set_permissions(source, perms);
        }
    }

    fs::rename(source, target).map_err(|e| AppError::AtomicWriteFailed(format!(
        "Failed to atomically replace {}: {}",
        target.display(),
        e
    )))?;

    Ok(())
}

// ──────────────────────────── Path Validation ────────────────────────────────

fn validate_path(path: &Path) -> AppResult<()> {
    let canonical = path
        .canonicalize()
        .map_err(|_| AppError::PathSafety(format!("Cannot resolve path: {}", path.display())))?;

    let path_str = canonical.to_string_lossy();

    #[cfg(windows)]
    {
        let lower = path_str.to_lowercase();
        if lower.contains(r"\windows\") || lower.contains(r"\program files\") {
            return Err(AppError::PathSafety(
                "Cannot modify files in system directories".to_string(),
            ));
        }
    }

    #[cfg(unix)]
    {
        if path_str.starts_with("/etc/")
            || path_str.starts_with("/usr/")
            || path_str.starts_with("/var/")
            || path_str.starts_with("/boot/")
            || path_str.starts_with("/proc/")
            || path_str.starts_with("/sys/")
        {
            return Err(AppError::PathSafety(
                "Cannot modify files in system directories".to_string(),
            ));
        }
    }

    Ok(())
}

// ──────────────────────────── Batch Cleaning ─────────────────────────────────

pub async fn clean_files_batch(
    file_paths: &[PathBuf],
    config: &CleanConfig,
    selections: &HashMap<String, Vec<MetadataItem>>,
    max_concurrency: usize,
) -> crate::types::BatchCleanResult {
    use futures::stream::{self, StreamExt};

    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(max_concurrency));

    let results: Vec<_> = stream::iter(file_paths.iter())
        .map(|path| {
            let config = config.clone();
            let selections = selections.clone();
            let semaphore = semaphore.clone();
            let path = path.clone();

            async move {
                let _permit = semaphore.acquire().await.unwrap();
                let items = selections
                    .get(&path.to_string_lossy().to_string())
                    .cloned()
                    .unwrap_or_default();
                clean_file(&path, &config, &items)
            }
        })
        .buffer_unordered(max_concurrency)
        .collect()
        .await;

    let mut total_cleaned = 0;
    let mut total_skipped = 0;
    let mut total_failed = 0;
    let mut warnings = Vec::new();
    let per_file_results: Vec<_> = results
        .into_iter()
        .map(|r| match r {
            Ok(result) => {
                match result.status {
                    CleanStatus::Cleaned | CleanStatus::DryRun => total_cleaned += 1,
                    CleanStatus::Skipped => total_skipped += 1,
                    CleanStatus::Failed => total_failed += 1,
                }
                if let Some(w) = &result.warning {
                    warnings.push(w.clone());
                }
                result
            }
            Err(e) => {
                total_failed += 1;
                CleanFileResult {
                    file_path: "unknown".to_string(),
                    status: CleanStatus::Failed,
                    backup_path: None,
                    metadata_removed_count: 0,
                    error: Some(e.to_user_message()),
                    warning: None,
                }
            }
        })
        .collect();

    crate::types::BatchCleanResult {
        total_files: file_paths.len(),
        files_cleaned: total_cleaned,
        files_skipped: total_skipped,
        files_failed: total_failed,
        dry_run: config.dry_run,
        per_file_results,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_file_consistency() {
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join("test_hash_consistency.txt");
        let mut file = fs::File::create(&temp_path).unwrap();
        writeln!(file, "test content").unwrap();
        drop(file);

        let hash1 = hash_file(&temp_path).unwrap();
        let hash2 = hash_file(&temp_path).unwrap();
        assert_eq!(hash1, hash2);
        assert!(!hash1.is_empty());

        let _ = fs::remove_file(&temp_path);
    }
}
