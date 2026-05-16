// src-tauri/src/metadata_scanner.rs
// Native metadata scanning for all supported formats.

use std::collections::HashMap;
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::errors::{AppError, AppResult};
use crate::types::{
    BatchScanResult, CategorySummary, FileScanResult, FileType, MetadataCategory, MetadataItem,
    RemovalCapability, SupportLevel,
};

// ──────────────────────────── Supported Extensions ───────────────────────────

const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "tiff", "tif", "webp", "bmp"];
const PDF_EXTENSIONS: &[&str] = &["pdf"];
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "ogg", "wav", "m4a", "aac", "wma"];
const VIDEO_EXTENSIONS: &[&str] = &["mp4", "avi", "mkv", "mov", "wmv", "webm", "m4v"];
const DOCUMENT_EXTENSIONS: &[&str] = &["docx", "xlsx", "pptx", "odt", "ods", "odp"];

const ALL_SUPPORTED: &[&str] = &[
    IMAGE_EXTENSIONS, PDF_EXTENSIONS, AUDIO_EXTENSIONS, VIDEO_EXTENSIONS, DOCUMENT_EXTENSIONS,
]
.concat();

pub fn detect_file_type(path: &Path) -> FileType {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        FileType::Image
    } else if PDF_EXTENSIONS.contains(&ext.as_str()) {
        FileType::Pdf
    } else if AUDIO_EXTENSIONS.contains(&ext.as_str()) {
        FileType::Audio
    } else if VIDEO_EXTENSIONS.contains(&ext.as_str()) {
        FileType::Video
    } else if DOCUMENT_EXTENSIONS.contains(&ext.as_str()) {
        FileType::Document
    } else {
        FileType::Other
    }
}

pub fn is_supported_extension(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    ALL_SUPPORTED.contains(&ext.as_str())
}

// ──────────────────────────── Directory Scanning ─────────────────────────────

pub fn collect_files(paths: &[String]) -> AppResult<Vec<PathBuf>> {
    let mut files = Vec::new();
    for path_str in paths {
        let path = PathBuf::from(path_str);
        if !path.exists() {
            return Err(AppError::FileNotFound(path_str.clone()));
        }
        if path.is_file() {
            if is_supported_extension(&path) {
                files.push(path);
            }
        } else if path.is_dir() {
            for entry in WalkDir::new(&path).follow_links(false) {
                let entry = entry.map_err(|e| AppError::Unknown(e.to_string()))?;
                if entry.file_type().is_file() && is_supported_extension(entry.path()) {
                    files.push(entry.path().to_path_buf());
                }
            }
        }
    }
    files.sort();
    Ok(files)
}

// ──────────────────────────── File Scanning ──────────────────────────────────

pub fn scan_file(file_path: &Path) -> FileScanResult {
    let file_type = detect_file_type(file_path);
    let file_size = fs::metadata(file_path).map(|m| m.len()).unwrap_or(0);
    let last_modified = fs::metadata(file_path)
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|t| chrono::DateTime::<chrono::Utc>::from(t));
    let path_str = file_path.to_string_lossy().to_string();

    match &file_type {
        FileType::Image => scan_image(file_path, &path_str, file_size, last_modified),
        FileType::Pdf => scan_pdf(file_path, &path_str, file_size, last_modified),
        FileType::Audio => scan_audio(file_path, &path_str, file_size, last_modified),
        FileType::Video => scan_video(file_path, &path_str, file_size, last_modified),
        FileType::Document => scan_document(file_path, &path_str, file_size, last_modified),
        FileType::Other => FileScanResult {
            file_path: path_str,
            file_type: FileType::Other,
            file_size_bytes: file_size,
            last_modified,
            support_level: SupportLevel::Unsupported,
            metadata_items: vec![],
            category_summary: vec![],
            errors: vec!["Unsupported file format".to_string()],
            warnings: vec![],
        },
    }
}

// ──────────────────── Image Scanning ────────────────────────────

fn scan_image(
    path: &Path,
    path_str: &str,
    file_size: u64,
    last_modified: Option<chrono::DateTime<chrono::Utc>>,
) -> FileScanResult {
    let mut items = Vec::new();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "jpg" | "jpeg" | "tiff" | "tif" => {
            if let Ok(exif_items) = scan_exif_native(path) {
                items.extend(exif_items);
            } else {
                errors.push("EXIF scan failed".to_string());
            }
        }
        "png" => match scan_png_text(path) {
            Ok(png_items) => items.extend(png_items),
            Err(e) => errors.push(format!("PNG text scan: {}", e)),
        },
        "webp" => match scan_webp_metadata(path) {
            Ok(webp_items) => items.extend(webp_items),
            Err(_) => warnings.push("WebP EXIF scanning requires exiftool".to_string()),
        },
        "bmp" => { /* BMP has no metadata */ }
        _ => {
            warnings.push("Unknown image format".to_string());
        }
    }

    let support_level = if !items.is_empty() {
        SupportLevel::Full
    } else {
        SupportLevel::Partial
    };

    FileScanResult {
        file_path: path_str.to_string(),
        file_type: FileType::Image,
        file_size_bytes: file_size,
        last_modified,
        support_level,
        metadata_items: items,
        category_summary: build_category_summary(&items),
        errors,
        warnings,
    }
}

fn scan_exif_native(path: &Path) -> AppResult<Vec<MetadataItem>> {
    use std::fs::File;
    let file = File::open(path).map_err(|e| AppError::FileRead {
        path: path.to_string_lossy().to_string(),
        source: e,
    })?;
    let mut bufreader = BufReader::new(&file);
    let exif_reader = exif::Reader::new();
    let exif = exif_reader.read_from_container(&mut bufreader).map_err(|e| {
        AppError::ExifParse(format!(
            "{}: {}",
            path.file_name().unwrap_or_default().to_string_lossy(),
            e
        ))
    })?;
    let mut items = Vec::new();
    for ifd in &[exif::In::PRIMARY, exif::In::EXIF, exif::In::GPS] {
        for f in exif.fields().filter(|f| f.ifd_num == *ifd) {
            let tag_name = f.tag.to_string();
            let value = f.display_value().with_unit(&exif).to_string();
            let category = if *ifd == exif::In::GPS {
                MetadataCategory::Gps
            } else {
                MetadataCategory::Exif
            };
            items.push(MetadataItem {
                key: tag_name,
                value: value.chars().take(200).collect(),
                category,
                capability: RemovalCapability::Removable,
                selected: true,
                warning: None,
            });
        }
    }
    Ok(items)
}

fn scan_png_text(path: &Path) -> AppResult<Vec<MetadataItem>> {
    let data = fs::read(path).map_err(|e| AppError::FileRead {
        path: path.to_string_lossy().to_string(),
        source: e,
    })?;
    if data.len() < 8 || &data[0..8] != b"\x89PNG\r\n\x1a\n" {
        return Ok(vec![]);
    }
    let mut items = Vec::new();
    let mut pos = 8;
    while pos + 8 <= data.len() {
        let length = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
            as usize;
        let chunk_type = &data[pos + 4..pos + 8];
        let chunk_type_str = String::from_utf8_lossy(chunk_type);

        if chunk_type_str == "tEXt" {
            let chunk_data = &data[pos + 8..pos + 8 + length.min(data.len() - pos - 8)];
            if let Some(null_pos) = chunk_data.iter().position(|&b| b == 0) {
                let keyword = String::from_utf8_lossy(&chunk_data[..null_pos]);
                let value = String::from_utf8_lossy(&chunk_data[null_pos + 1..]);
                items.push(MetadataItem {
                    key: format!("PNG:tEXt:{}", keyword),
                    value: value.chars().take(200).collect(),
                    category: MetadataCategory::Exif,
                    capability: RemovalCapability::Removable,
                    selected: true,
                    warning: None,
                });
            }
        } else if chunk_type_str == "iTXt" {
            let chunk_data = &data[pos + 8..pos + 8 + length.min(data.len() - pos - 8)];
            if let Some(null_pos) = chunk_data.iter().position(|&b| b == 0) {
                let keyword = String::from_utf8_lossy(&chunk_data[..null_pos]);
                let rest = &chunk_data[null_pos + 1..];
                if rest.len() >= 2 {
                    let after_flags = &rest[2..];
                    if let Some(null2) = after_flags.iter().position(|&b| b == 0) {
                        let after_lang = &after_flags[null2 + 1..];
                        if let Some(null3) = after_lang.iter().position(|&b| b == 0) {
                            let text = String::from_utf8_lossy(&after_lang[null3 + 1..]);
                            items.push(MetadataItem {
                                key: format!("PNG:iTXt:{}", keyword),
                                value: text.chars().take(200).collect(),
                                category: MetadataCategory::Exif,
                                capability: RemovalCapability::Removable,
                                selected: true,
                                warning: None,
                            });
                        }
                    }
                }
            }
        } else if chunk_type_str == "IEND" {
            break;
        }
        pos += 12 + length;
    }
    Ok(items)
}

fn scan_webp_metadata(path: &Path) -> AppResult<Vec<MetadataItem>> {
    let data = fs::read(path).map_err(|e| AppError::FileRead {
        path: path.to_string_lossy().to_string(),
        source: e,
    })?;
    if data.len() < 16 || &data[0..4] != b"RIFF" || &data[8..12] != b"WEBP" {
        return Ok(vec![]);
    }
    let mut items = Vec::new();
    let mut pos = 12;
    while pos + 8 <= data.len() {
        let chunk_size = u32::from_le_bytes([
            data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7],
        ]) as usize;
        let fourcc_str = String::from_utf8_lossy(&data[pos..pos + 4]);
        if fourcc_str == "EXIF" {
            items.push(MetadataItem {
                key: "WebP:EXIF".to_string(),
                value: "EXIF chunk present".to_string(),
                category: MetadataCategory::Exif,
                capability: RemovalCapability::Removable,
                selected: true,
                warning: None,
            });
        } else if fourcc_str == "XMP " {
            items.push(MetadataItem {
                key: "WebP:XMP".to_string(),
                value: "XMP chunk present".to_string(),
                category: MetadataCategory::Xmp,
                capability: RemovalCapability::Removable,
                selected: true,
                warning: None,
            });
        } else if fourcc_str == "ICCP" {
            items.push(MetadataItem {
                key: "WebP:ICCP".to_string(),
                value: "ICC Profile chunk present".to_string(),
                category: MetadataCategory::Exif,
                capability: RemovalCapability::Removable,
                selected: true,
                warning: Some("Removing ICC profile may affect color accuracy".to_string()),
            });
        }
        pos += 8 + chunk_size + (chunk_size % 2);
    }
    Ok(items)
}

// ──────────────────── PDF Scanning ────────────────────────────

fn scan_pdf(
    path: &Path,
    path_str: &str,
    file_size: u64,
    last_modified: Option<chrono::DateTime<chrono::Utc>>,
) -> FileScanResult {
    let mut items = Vec::new();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    match lopdf::Document::load(path) {
        Ok(doc) => {
            if let Ok(info) = doc.trailer.get(b"Info") {
                let info_obj_id = match info {
                    lopdf::Object::Reference(r) => r.0,
                    _ => {
                        errors.push("PDF Info dictionary not found".to_string());
                        0
                    }
                };
                if info_obj_id > 0 {
                    if let Ok(info_dict) = doc.get_object((info_obj_id, 0)) {
                        if let lopdf::Object::Dictionary(dict) = info_dict {
                            scan_pdf_dict(&dict, &mut items);
                        }
                    }
                }
            }
            if let Ok(xmp) = doc.get_xmp_metadata() {
                if !xmp.is_empty() {
                    items.push(MetadataItem {
                        key: "XMP Metadata".to_string(),
                        value: "XMP metadata stream detected".to_string(),
                        category: MetadataCategory::Xmp,
                        capability: RemovalCapability::Partial,
                        selected: true,
                        warning: Some("XMP removal may affect PDF/A compliance".to_string()),
                    });
                }
            }
            if has_embedded_files(&doc) {
                warnings.push("PDF contains embedded objects — deep cleaning is not supported. Embedded metadata will not be removed.".to_string());
            }
        }
        Err(e) => {
            errors.push(format!("Failed to parse PDF: {}", e));
        }
    }

    let support_level = if !items.is_empty() {
        SupportLevel::Full
    } else if errors.is_empty() {
        SupportLevel::Unsupported
    } else {
        SupportLevel::Partial
    };

    FileScanResult {
        file_path: path_str.to_string(),
        file_type: FileType::Pdf,
        file_size_bytes: file_size,
        last_modified,
        support_level,
        metadata_items: items,
        category_summary: build_category_summary(&items),
        errors,
        warnings,
    }
}

fn scan_pdf_dict(dict: &lopdf::Dictionary, items: &mut Vec<MetadataItem>) {
    let pdf_keys = [
        (b"Author", "Author"),
        (b"Creator", "Creator"),
        (b"Producer", "Producer"),
        (b"Title", "Title"),
        (b"Subject", "Subject"),
        (b"Keywords", "Keywords"),
        (b"CreationDate", "Creation Date"),
        (b"ModDate", "Modification Date"),
    ];
    for (key, label) in pdf_keys.iter() {
        if let Ok(obj) = dict.get(*key) {
            let value = match obj {
                lopdf::Object::String(s, _) => String::from_utf8_lossy(s).to_string(),
                lopdf::Object::Name(n) => String::from_utf8_lossy(n).to_string(),
                _ => format!("{:?}", obj),
            };
            items.push(MetadataItem {
                key: label.to_string(),
                value: value.chars().take(200).collect(),
                category: MetadataCategory::PdfInfo,
                capability: RemovalCapability::Removable,
                selected: true,
                warning: None,
            });
        }
    }
}

fn has_embedded_files(doc: &lopdf::Document) -> bool {
    doc.trailer
        .get(b"Names")
        .ok()
        .and_then(|obj| {
            if let lopdf::Object::Dictionary(d) = obj {
                d.get(b"EmbeddedFiles").ok()
            } else {
                None
            }
        })
        .is_some()
}

// ──────────────────── Audio Scanning ────────────────────────────

fn scan_audio(
    path: &Path,
    path_str: &str,
    file_size: u64,
    last_modified: Option<chrono::DateTime<chrono::Utc>>,
) -> FileScanResult {
    let mut items = Vec::new();
    let mut errors = Vec::new();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "mp3" => match id3::Tag::read_from_path(path) {
            Ok(tag) => extract_id3_tags(&tag, &mut items),
            Err(e) => errors.push(format!("ID3 read: {}", e)),
        },
        "flac" => match scan_flac_vorbis_comments(path) {
            Ok(flac_items) => items.extend(flac_items),
            Err(e) => errors.push(format!("FLAC scan: {}", e)),
        },
        "ogg" => match scan_ogg_vorbis_comments(path) {
            Ok(ogg_items) => items.extend(ogg_items),
            Err(_) => errors.push("OGG scan failed".to_string()),
        },
        "m4a" | "aac" => match scan_mp4_metadata(path) {
            Ok(mp4_items) => items.extend(mp4_items),
            Err(_) => errors.push("M4A scan failed".to_string()),
        },
        "wav" => match scan_wav_info(path) {
            Ok(wav_items) => items.extend(wav_items),
            Err(e) => errors.push(format!("WAV scan: {}", e)),
        },
        "wma" => {
            errors.push("WMA metadata scanning requires exiftool or ffmpeg".to_string());
        }
        _ => {
            errors.push(format!("Unsupported audio format: {}", ext));
        }
    }

    let support_level = if !items.is_empty() {
        SupportLevel::Full
    } else {
        SupportLevel::Partial
    };

    FileScanResult {
        file_path: path_str.to_string(),
        file_type: FileType::Audio,
        file_size_bytes: file_size,
        last_modified,
        support_level,
        metadata_items: items,
        category_summary: build_category_summary(&items),
        errors,
        warnings: vec![],
    }
}

fn extract_id3_tags(tag: &id3::Tag, items: &mut Vec<MetadataItem>) {
    let id3_fields: Vec<(&str, &str)> = vec![
        ("title", "Title"),
        ("artist", "Artist"),
        ("album", "Album"),
        ("genre", "Genre"),
        ("year", "Year"),
        ("track", "Track"),
        ("comment", "Comment"),
    ];
    for (getter, label) in id3_fields {
        let value = match getter {
            "title" => tag.title().map(|s| s.to_string()),
            "artist" => tag.artist().map(|s| s.to_string()),
            "album" => tag.album().map(|s| s.to_string()),
            "genre" => tag.genre().map(|s| s.to_string()),
            "year" => tag.year().map(|y| y.to_string()),
            "track" => tag.track().map(|t| t.to_string()),
            "comment" => tag.comment().map(|c| c.to_string()),
            _ => None,
        };
        if let Some(val) = value {
            items.push(MetadataItem {
                key: label.to_string(),
                value: val.chars().take(200).collect(),
                category: MetadataCategory::Id3Tags,
                capability: RemovalCapability::Removable,
                selected: true,
                warning: None,
            });
        }
    }
}

fn scan_flac_vorbis_comments(path: &Path) -> AppResult<Vec<MetadataItem>> {
    use std::fs::File;
    let file = File::open(path).map_err(|e| AppError::FileRead {
        path: path.to_string_lossy().to_string(),
        source: e,
    })?;
    let mut reader = metaflac::Reader::new(file);
    for block in reader.blocks() {
        if let metaflac::Block::VorbisComment(vc) = block.map_err(|e| {
            AppError::Unknown(format!("FLAC block read error: {}", e))
        })? {
            let mut items = Vec::new();
            for (key, values) in vc.comments.iter() {
                for value in values {
                    items.push(MetadataItem {
                        key: format!("FLAC:{}", key),
                        value: value.chars().take(200).collect(),
                        category: MetadataCategory::Id3Tags,
                        capability: RemovalCapability::Removable,
                        selected: true,
                        warning: None,
                    });
                }
            }
            return Ok(items);
        }
    }
    Ok(vec![])
}

fn scan_ogg_vorbis_comments(path: &Path) -> AppResult<Vec<MetadataItem>> {
    use std::fs::File;
    use std::io::Read;
    let mut file = File::open(path).map_err(|e| AppError::FileRead {
        path: path.to_string_lossy().to_string(),
        source: e,
    })?;
    let mut oggs = [0u8; 4];
    file.read_exact(&mut oggs).map_err(|_| {
        AppError::CorruptedFile("Not a valid OGG file".to_string())
    })?;
    if &oggs != b"OggS" {
        return Ok(vec![]);
    }
    let mut skip = [0u8; 22];
    file.read_exact(&mut skip).map_err(|e| {
        AppError::CorruptedFile(format!("OGG header read error: {}", e))
    })?;
    let segment_count = {
        let mut buf = [0u8; 1];
        file.read_exact(&mut buf).map_err(|e| {
            AppError::CorruptedFile(format!("OGG segment count error: {}", e))
        })?;
        buf[0] as usize
    };
    let mut segment_table = vec![0u8; segment_count];
    file.read_exact(&mut segment_table).map_err(|e| {
        AppError::CorruptedFile(format!("OGG segment table error: {}", e))
    })?;
    let page_size: usize = segment_table.iter().map(|&s| s as usize).sum();
    let mut page_data = vec![0u8; page_size];
    file.read_exact(&mut page_data).map_err(|e| {
        AppError::CorruptedFile(format!("OGG page data error: {}", e))
    })?;

    // Second page
    let mut oggs2 = [0u8; 4];
    if file.read_exact(&mut oggs2).is_err() || &oggs2 != b"OggS" {
        return Ok(vec![]);
    }
    let mut skip2 = [0u8; 22];
    file.read_exact(&mut skip2).map_err(|_| {
        AppError::CorruptedFile("OGG second page read error".to_string())
    })?;
    let segment_count2 = {
        let mut buf = [0u8; 1];
        file.read_exact(&mut buf).map_err(|_| {
            AppError::CorruptedFile("OGG second page segment count error".to_string())
        })?;
        buf[0] as usize
    };
    let mut segment_table2 = vec![0u8; segment_count2];
    file.read_exact(&mut segment_table2).map_err(|_| {
        AppError::CorruptedFile("OGG second page segment table error".to_string())
    })?;
    let page_size2: usize = segment_table2.iter().map(|&s| s as usize).sum();
    let mut page_data2 = vec![0u8; page_size2];
    file.read_exact(&mut page_data2).map_err(|_| {
        AppError::CorruptedFile("OGG second page data error".to_string())
    })?;

    Ok(parse_vorbis_comments_raw(&page_data2))
}

fn parse_vorbis_comments_raw(data: &[u8]) -> Vec<MetadataItem> {
    let mut items = Vec::new();
    if data.len() > 12 && &data[0..8] == b"OpusTags" {
        return parse_vorbis_comment_block(&data[8..]);
    }
    if data.len() > 10 && &data[1..7] == b"vorbis" && data.len() > 7 {
        return parse_vorbis_comment_block(&data[7..]);
    }
    items
}

fn parse_vorbis_comment_block(data: &[u8]) -> Vec<MetadataItem> {
    let mut items = Vec::new();
    if data.len() < 8 {
        return items;
    }
    let vendor_len = lebe::Get::get(&data[0..4]).unwrap_or((0, 4)).0 as usize;
    let offset = 4 + vendor_len;
    if offset + 4 > data.len() {
        return items;
    }
    let comment_count = lebe::Get::get(&data[offset..offset + 4]).unwrap_or((0, offset + 4)).0;
    let mut pos = offset + 4;
    for _ in 0..comment_count {
        if pos + 4 > data.len() {
            break;
        }
        let (comment_len, consumed) = lebe::Get::get(&data[pos..pos + 4]).unwrap_or((0, 4));
        pos += consumed;
        if pos + comment_len as usize > data.len() {
            break;
        }
        let comment = String::from_utf8_lossy(&data[pos..pos + comment_len as usize]);
        pos += comment_len as usize;
        if let Some(eq_pos) = comment.find('=') {
            let key = &comment[..eq_pos];
            let value = &comment[eq_pos + 1..];
            items.push(MetadataItem {
                key: format!("OGG:{}", key),
                value: value.chars().take(200).collect(),
                category: MetadataCategory::Id3Tags,
                capability: RemovalCapability::Removable,
                selected: true,
                warning: None,
            });
        }
    }
    items
}

fn scan_mp4_metadata(path: &Path) -> AppResult<Vec<MetadataItem>> {
    use std::fs::File;
    use std::io::Read;
    let mut file = File::open(path).map_err(|e| AppError::FileRead {
        path: path.to_string_lossy().to_string(),
        source: e,
    })?;
    let mut data = Vec::new();
    file.read_to_end(&mut data).map_err(|e| AppError::FileRead {
        path: path.to_string_lossy().to_string(),
        source: e,
    })?;

    let mp4_keys: [(&[u8], &str); 11] = [
        (b"\xA9nam", "Title"),
        (b"\xA9ART", "Artist"),
        (b"\xA9alb", "Album"),
        (b"\xA9gen", "Genre"),
        (b"\xA9day", "Year"),
        (b"\xA9cmt", "Comment"),
        (b"\xA9wrt", "Composer"),
        (b"\xA9too", "Encoder"),
        (b"\xA9grp", "Grouping"),
        (b"desc", "Description"),
        (b"ldes", "Long Description"),
    ];
    let mut items = Vec::new();
    for (atom_bytes, label) in mp4_keys.iter() {
        let mut search_pos = 0;
        while let Some(pos) = data[search_pos..].windows(4).position(|w| w == *atom_bytes) {
            let atom_pos = search_pos + pos;
            if atom_pos >= 4 {
                let size_bytes = &data[atom_pos - 4..atom_pos];
                let atom_size = u32::from_be_bytes([
                    size_bytes[0], size_bytes[1], size_bytes[2], size_bytes[3],
                ]) as usize;
                if atom_size > 8 && atom_pos + atom_size - 8 <= data.len() {
                    let value_data = &data[atom_pos + 4..atom_pos + atom_size - 8];
                    if value_data.len() > 16 {
                        let actual = String::from_utf8_lossy(&value_data[16..]);
                        if !actual.is_empty()
                            && actual.chars().all(|c| !c.is_control() || c == '\n')
                        {
                            items.push(MetadataItem {
                                key: format!("MP4:{}", label),
                                value: actual.chars().take(200).collect(),
                                category: MetadataCategory::Id3Tags,
                                capability: RemovalCapability::Removable,
                                selected: true,
                                warning: None,
                            });
                            break;
                        }
                    }
                }
            }
            search_pos = atom_pos + 1;
        }
    }
    Ok(items)
}

/// Scan WAV RIFF INFO chunk for metadata.
/// RIFF format: "RIFF" + size(4) + "WAVE" + chunks
/// INFO is stored as a LIST chunk with type "INFO".
fn scan_wav_info(path: &Path) -> AppResult<Vec<MetadataItem>> {
    use std::fs::File;
    use std::io::Read;
    let mut file = File::open(path).map_err(|e| AppError::FileRead {
        path: path.to_string_lossy().to_string(),
        source: e,
    })?;
    let mut data = Vec::new();
    file.read_to_end(&mut data).map_err(|e| AppError::FileRead {
        path: path.to_string_lossy().to_string(),
        source: e,
    })?;

    // Verify RIFF WAVE header
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return Ok(vec![]);
    }

    let mut items = Vec::new();
    let mut pos = 12;

    // WAV INFO tags mapping
    let info_keys: [(&[u8], &str); 14] = [
        (b"INAM", "Title"),
        (b"IART", "Artist"),
        (b"IPRD", "Album"),
        (b"ICMT", "Comment"),
        (b"ICRD", "Date"),
        (b"IGNR", "Genre"),
        (b"ISFT", "Software"),
        (b"IENG", "Engineer"),
        (b"ICOP", "Copyright"),
        (b"ISBJ", "Subject"),
        (b"ISRC", "Source"),
        (b"ITCH", "Technician"),
        (b"IARL", "Archival Location"),
        (b"ICMS", "Commissioned"),
    ];

    while pos + 8 <= data.len() {
        let chunk_id = &data[pos..pos + 4];
        let chunk_size = u32::from_le_bytes([
            data[pos + 4],
            data[pos + 5],
            data[pos + 6],
            data[pos + 7],
        ]) as usize;

        // Check for LIST chunk with INFO type
        if chunk_id == b"LIST" && chunk_size >= 4 {
            let list_type = &data[pos + 8..pos + 12];
            if list_type == b"INFO" {
                // Parse sub-chunks within the LIST/INFO block
                let mut sub_pos = pos + 12;
                let list_end = pos + 8 + chunk_size;
                while sub_pos + 8 <= list_end && sub_pos + 8 <= data.len() {
                    let sub_id = &data[sub_pos..sub_pos + 4];
                    let sub_size = u32::from_le_bytes([
                        data[sub_pos + 4],
                        data[sub_pos + 5],
                        data[sub_pos + 6],
                        data[sub_pos + 7],
                    ]) as usize;

                    for (tag, label) in info_keys.iter() {
                        if sub_id == *tag && sub_size > 0 {
                            let value_start = sub_pos + 8;
                            if value_start + sub_size <= data.len() {
                                let value_bytes = &data[value_start..value_start + sub_size];
                                // Null-terminate strings, strip trailing null
                                let value = String::from_utf8_lossy(value_bytes)
                                    .trim_end_matches('\0')
                                    .to_string();
                                if !value.is_empty() {
                                    items.push(MetadataItem {
                                        key: format!("WAV:{}", label),
                                        value: value.chars().take(200).collect(),
                                        category: MetadataCategory::Id3Tags,
                                        capability: RemovalCapability::Removable,
                                        selected: true,
                                        warning: None,
                                    });
                                }
                            }
                        }
                    }
                    sub_pos += 8 + sub_size + (sub_size % 2); // padding to even
                }
            }
        }

        pos += 8 + chunk_size + (chunk_size % 2);
    }

    Ok(items)
}

// ──────────────────── Video Scanning ────────────────────────────

fn scan_video(
    path: &Path,
    path_str: &str,
    file_size: u64,
    last_modified: Option<chrono::DateTime<chrono::Utc>>,
) -> FileScanResult {
    let mut items = Vec::new();
    let mut errors = Vec::new();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "mp4" | "m4v" => match scan_mp4_metadata(path) {
            Ok(mp4_items) => {
                if !mp4_items.is_empty() {
                    items.extend(mp4_items);
                } else {
                    errors.push("No MP4 metadata atoms found".to_string());
                }
            }
            Err(e) => errors.push(format!("MP4 metadata scan: {}", e)),
        },
        "mov" => match scan_mp4_metadata(path) {
            Ok(mov_items) => items.extend(mov_items),
            Err(_) => errors.push("MOV scanning failed".to_string()),
        },
        "avi" => match scan_avi_info(path) {
            Ok(avi_items) => items.extend(avi_items),
            Err(e) => errors.push(format!("AVI scan: {}", e)),
        },
        "mkv" | "webm" => match scan_mkv_tags(path) {
            Ok(mkv_items) => items.extend(mkv_items),
            Err(e) => errors.push(format!("MKV/WebM scan: {}", e)),
        },
        "wmv" => {
            errors.push("WMV scanning requires ffmpeg".to_string());
        }
        _ => {
            errors.push(format!("Unsupported video format: {}", ext));
        }
    }

    let support_level = if !items.is_empty() {
        SupportLevel::Full
    } else {
        SupportLevel::Partial
    };

    FileScanResult {
        file_path: path_str.to_string(),
        file_type: FileType::Video,
        file_size_bytes: file_size,
        last_modified,
        support_level,
        metadata_items: items,
        category_summary: build_category_summary(&items),
        errors,
        warnings: if support_level == SupportLevel::Partial {
            vec!["Install ffmpeg for full video metadata scanning".to_string()]
        } else {
            vec![]
        },
    }
}

/// Scan AVI RIFF INFO chunk for metadata.
/// AVI is RIFF-based: "RIFF" + size + "AVI " + chunks
/// INFO is stored in a LIST chunk with type "INFO", same as WAV.
fn scan_avi_info(path: &Path) -> AppResult<Vec<MetadataItem>> {
    use std::fs::File;
    use std::io::Read;
    let mut file = File::open(path).map_err(|e| AppError::FileRead {
        path: path.to_string_lossy().to_string(),
        source: e,
    })?;
    let mut data = Vec::new();
    file.read_to_end(&mut data).map_err(|e| AppError::FileRead {
        path: path.to_string_lossy().to_string(),
        source: e,
    })?;

    // Verify RIFF AVI header
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"AVI " {
        // Some AVIs use "AVIX" instead
        if data.len() >= 12 && &data[8..12] != b"AVIX" {
            return Ok(vec![]);
        }
    }

    let mut items = Vec::new();
    let mut pos = 12;

    let info_keys: [(&[u8], &str); 14] = [
        (b"INAM", "Title"),
        (b"IART", "Artist"),
        (b"IPRD", "Product"),
        (b"ICMT", "Comment"),
        (b"ICRD", "Date"),
        (b"IGNR", "Genre"),
        (b"ISFT", "Software"),
        (b"IENG", "Engineer"),
        (b"ICOP", "Copyright"),
        (b"ISBJ", "Subject"),
        (b"ISRC", "Source"),
        (b"ITCH", "Technician"),
        (b"IARL", "Archival Location"),
        (b"ICMS", "Commissioned"),
    ];

    while pos + 8 <= data.len() {
        let chunk_id = &data[pos..pos + 4];
        let chunk_size = u32::from_le_bytes([
            data[pos + 4],
            data[pos + 5],
            data[pos + 6],
            data[pos + 7],
        ]) as usize;

        if chunk_id == b"LIST" && chunk_size >= 4 {
            let list_type = &data[pos + 8..pos + 12];
            if list_type == b"INFO" {
                let mut sub_pos = pos + 12;
                let list_end = pos + 8 + chunk_size;
                while sub_pos + 8 <= list_end && sub_pos + 8 <= data.len() {
                    let sub_id = &data[sub_pos..sub_pos + 4];
                    let sub_size = u32::from_le_bytes([
                        data[sub_pos + 4],
                        data[sub_pos + 5],
                        data[sub_pos + 6],
                        data[sub_pos + 7],
                    ]) as usize;

                    for (tag, label) in info_keys.iter() {
                        if sub_id == *tag && sub_size > 0 {
                            let value_start = sub_pos + 8;
                            if value_start + sub_size <= data.len() {
                                let value = String::from_utf8_lossy(
                                    &data[value_start..value_start + sub_size],
                                )
                                .trim_end_matches('\0')
                                .to_string();
                                if !value.is_empty() {
                                    items.push(MetadataItem {
                                        key: format!("AVI:{}", label),
                                        value: value.chars().take(200).collect(),
                                        category: MetadataCategory::VideoContainer,
                                        capability: RemovalCapability::Removable,
                                        selected: true,
                                        warning: None,
                                    });
                                }
                            }
                        }
                    }
                    sub_pos += 8 + sub_size + (sub_size % 2);
                }
            }
        }
        pos += 8 + chunk_size + (chunk_size % 2);
    }

    Ok(items)
}

/// Scan MKV/WebM EBML container for Tags elements.
/// EBML format: ID (variable length) + data size (variable length) + data
/// We look for Tags elements (ID 0x1254) containing SimpleTag elements.
fn scan_mkv_tags(path: &Path) -> AppResult<Vec<MetadataItem>> {
    use std::fs::File;
    use std::io::Read;
    let mut file = File::open(path).map_err(|e| AppError::FileRead {
        path: path.to_string_lossy().to_string(),
        source: e,
    })?;
    let mut data = Vec::new();
    // Only read first 10MB — Tags are usually near the end,
    // but for scanning we check what we can.
    let max_read = 10 * 1024 * 1024;
    let bytes_read = file
        .take(max_read as u64)
        .read_to_end(&mut data)
        .map_err(|e| AppError::FileRead {
            path: path.to_string_lossy().to_string(),
            source: e,
        })?;

    if bytes_read < 4 {
        return Ok(vec![]);
    }

    // Verify EBML header: 0x1A45DFA3
    if data[0] != 0x1A || data[1] != 0x45 || data[2] != 0xDF || data[3] != 0xA3 {
        return Ok(vec![]);
    }

    let mut items = Vec::new();

    // Search for Tags element (ID: 0x12 0x54)
    let mut search_pos = 0;
    while search_pos + 2 < data.len() {
        // Look for Tags element ID bytes
        if data[search_pos] == 0x12 && data[search_pos + 1] == 0x54 {
            // Found potential Tags element
            let tag_id_pos = search_pos;
            // Parse element size after ID
            let size_offset = tag_id_pos + 2;
            if size_offset >= data.len() {
                break;
            }

            if let Some((elem_size, header_len)) = parse_ebml_variable_size(&data, size_offset) {
                let elem_start = size_offset + header_len;
                if elem_start + elem_size as usize > data.len() {
                    // Tags element extends beyond what we read — search for it later
                    search_pos += 1;
                    continue;
                }

                // Parse SimpleTag elements within Tags
                parse_ebml_tags_segment(
                    &data[elem_start..elem_start + elem_size as usize],
                    &mut items,
                );

                if !items.is_empty() {
                    return Ok(items);
                }
            }
        }
        search_pos += 1;
    }

    // Also check Segment Info for Title
    // Segment element ID: 0x18 0x53 0x80 0x67
    search_pos = 0;
    while search_pos + 4 < data.len() {
        if data[search_pos] == 0x18
            && data[search_pos + 1] == 0x53
            && data[search_pos + 2] == 0x80
            && data[search_pos + 3] == 0x67
        {
            // Found Segment, look for Info inside
            // This is a simplified search — full EBML parsing is much more complex
            break;
        }
        search_pos += 1;
    }

    Ok(items)
}

/// Parse EBML variable-size integer.
/// First byte: 0xxxxxxx = 1 byte, 10xxxxxx = 2 bytes, 110xxxxx = 3 bytes, etc.
fn parse_ebml_variable_size(data: &[u8], offset: usize) -> Option<(u64, usize)> {
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

    let mut value = (first & (0xFF >> size_len)) as u64;
    for i in 1..size_len {
        value = (value << 8) | data[offset + i] as u64;
    }
    Some((value, size_len))
}

/// Parse SimpleTag elements within a Tags segment.
fn parse_ebml_tags_segment(data: &[u8], items: &mut Vec<MetadataItem>) {
    let mut pos = 0;
    while pos < data.len() {
        if pos + 2 > data.len() {
            break;
        }
        // SimpleTag element ID: 0x67 0xC8
        if data[pos] == 0x67 && data[pos + 1] == 0xC8 {
            let size_offset = pos + 2;
            if let Some((elem_size, header_len)) = parse_ebml_variable_size(data, size_offset) {
                let elem_start = size_offset + header_len;
                let elem_end = elem_start + elem_size as usize;
                if elem_end <= data.len() {
                    parse_simple_tag(&data[elem_start..elem_end], items);
                }
                pos = elem_end;
                continue;
            }
        }
        pos += 1;
    }
}

/// Parse a single SimpleTag element.
fn parse_simple_tag(data: &[u8], items: &mut Vec<MetadataItem>) {
    let mut pos = 0;
    let mut tag_name = String::new();
    let mut tag_value = String::new();

    while pos < data.len() {
        if pos >= data.len() {
            break;
        }
        // TagName element: ID 0x45 0xA3
        if pos + 1 < data.len()
            && data[pos] == 0x45
            && data[pos + 1] == 0xA3
        {
            if let Some((name_size, name_header)) =
                parse_ebml_variable_size(data, pos + 2)
            {
                let name_start = pos + 2 + name_header;
                let name_end = name_start + name_size as usize;
                if name_end <= data.len() {
                    tag_name = String::from_utf8_lossy(&data[name_start..name_end])
                        .to_string();
                }
                pos = name_end;
                continue;
            }
        }
        // TagString element: ID 0x44 0x87
        if pos + 1 < data.len()
            && data[pos] == 0x44
            && data[pos + 1] == 0x87
        {
            if let Some((val_size, val_header)) =
                parse_ebml_variable_size(data, pos + 2)
            {
                let val_start = pos + 2 + val_header;
                let val_end = val_start + val_size as usize;
                if val_end <= data.len() {
                    tag_value = String::from_utf8_lossy(&data[val_start..val_end])
                        .to_string();
                }
                pos = val_end;
                continue;
            }
        }
        pos += 1;
    }

    if !tag_name.is_empty() && !tag_value.is_empty() {
        items.push(MetadataItem {
            key: format!("MKV:{}", tag_name),
            value: tag_value.chars().take(200).collect(),
            category: MetadataCategory::VideoContainer,
            capability: RemovalCapability::Removable,
            selected: true,
            warning: None,
        });
    }
}

// ──────────────────── Document Scanning ────────────────────────────

fn scan_document(
    path: &Path,
    path_str: &str,
    file_size: u64,
    last_modified: Option<chrono::DateTime<chrono::Utc>>,
) -> FileScanResult {
    let mut items = Vec::new();
    let mut errors = Vec::new();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "docx" | "xlsx" | "pptx" => match scan_office_metadata(path) {
            Ok(office_items) => items.extend(office_items),
            Err(e) => errors.push(format!("Office doc scan: {}", e)),
        },
        "odt" | "ods" | "odp" => match scan_odf_metadata(path) {
            Ok(odf_items) => items.extend(odf_items),
            Err(e) => errors.push(format!("ODF doc scan: {}", e)),
        },
        _ => {
            errors.push(format!("Unsupported document format: {}", ext));
        }
    }

    let support_level = if !items.is_empty() {
        SupportLevel::Full
    } else {
        SupportLevel::Partial
    };

    FileScanResult {
        file_path: path_str.to_string(),
        file_type: FileType::Document,
        file_size_bytes: file_size,
        last_modified,
        support_level,
        metadata_items: items,
        category_summary: build_category_summary(&items),
        errors,
        warnings: vec![],
    }
}

fn scan_office_metadata(path: &Path) -> AppResult<Vec<MetadataItem>> {
    use std::io::Read;
    let mut archive = zip::ZipArchive::new(
        std::fs::File::open(path).map_err(|e| AppError::FileRead {
            path: path.to_string_lossy().to_string(),
            source: e,
        })?,
    )
    .map_err(|e| AppError::Unknown(format!("ZIP open error: {}", e)))?;

    let mut items = Vec::new();
    let core_xml = match archive.by_name("docProps/core.xml") {
        Ok(mut f) => {
            let mut content = String::new();
            f.read_to_string(&mut content)
                .map_err(|e| AppError::Unknown(format!("core.xml read error: {}", e)))?;
            content
        }
        Err(_) => return Ok(items),
    };

    let xml_str = core_xml.as_str();
    let props = [
        ("dc:title", "Title"),
        ("dc:creator", "Author"),
        ("dc:subject", "Subject"),
        ("dc:description", "Description"),
        ("cp:keywords", "Keywords"),
        ("cp:category", "Category"),
        ("cp:lastModifiedBy", "Last Modified By"),
        ("cp:revision", "Revision"),
    ];

    for (xml_tag, label) in props.iter() {
        let open_tag = format!("<{}", xml_tag);
        let close_tag = format!("</{}>", xml_tag);
        if let Some(start) = xml_str.find(&open_tag) {
            if let Some(tag_end) = xml_str[start..].find('>') {
                let value_start = start + tag_end + 1;
                if let Some(end) = xml_str[value_start..].find(&close_tag) {
                    let value = xml_str[value_start..value_start + end].trim();
                    if !value.is_empty() {
                        items.push(MetadataItem {
                            key: format!("Office:{}", label),
                            value: value.chars().take(200).collect(),
                            category: MetadataCategory::DocumentProperties,
                            capability: RemovalCapability::Removable,
                            selected: true,
                            warning: None,
                        });
                    }
                }
            }
        }
    }
    Ok(items)
}

fn scan_odf_metadata(path: &Path) -> AppResult<Vec<MetadataItem>> {
    use std::io::Read;
    let mut archive = zip::ZipArchive::new(
        std::fs::File::open(path).map_err(|e| AppError::FileRead {
            path: path.to_string_lossy().to_string(),
            source: e,
        })?,
    )
    .map_err(|e| AppError::Unknown(format!("ZIP open error: {}", e)))?;

    let mut items = Vec::new();
    let meta_xml = match archive.by_name("meta.xml") {
        Ok(mut f) => {
            let mut content = String::new();
            f.read_to_string(&mut content).map_err(|e| {
                AppError::Unknown(format!("meta.xml read error: {}", e))
            })?;
            content
        }
        Err(_) => return Ok(items),
    };

    let xml_str = meta_xml.as_str();
    let props = [
        ("dc:title", "Title"),
        ("dc:creator", "Author"),
        ("dc:description", "Description"),
        ("meta:keyword", "Keywords"),
        ("meta:generator", "Generator"),
        ("meta:editing-duration", "Editing Duration"),
        ("meta:editing-cycles", "Editing Cycles"),
        ("meta:initial-creator", "Initial Creator"),
        ("meta:print-date", "Print Date"),
        ("meta:creation-date", "Creation Date"),
        ("meta:date", "Modification Date"),
    ];

    for (xml_tag, label) in props.iter() {
        let open_tag = format!("<{}", xml_tag);
        let close_tag = format!("</{}>", xml_tag);
        if let Some(start) = xml_str.find(&open_tag) {
            if let Some(tag_end) = xml_str[start..].find('>') {
                let value_start = start + tag_end + 1;
                if let Some(end) = xml_str[value_start..].find(&close_tag) {
                    let value = xml_str[value_start..value_start + end].trim();
                    if !value.is_empty() {
                        items.push(MetadataItem {
                            key: format!("ODF:{}", label),
                            value: value.chars().take(200).collect(),
                            category: MetadataCategory::DocumentProperties,
                            capability: RemovalCapability::Removable,
                            selected: true,
                            warning: None,
                        });
                    }
                }
            }
        }
    }

    if xml_str.contains("meta:user-defined") {
        items.push(MetadataItem {
            key: "ODF:UserDefined".to_string(),
            value: "Custom metadata present".to_string(),
            category: MetadataCategory::DocumentProperties,
            capability: RemovalCapability::Partial,
            selected: true,
            warning: Some(
                "User-defined metadata may contain custom properties".to_string(),
            ),
        });
    }

    Ok(items)
}

// ──────────────────────────── Batch Scanning ─────────────────────────────────

pub fn scan_files(file_paths: &[PathBuf]) -> BatchScanResult {
    let mut files = Vec::new();
    let mut total_with_metadata = 0;
    let mut total_errors = 0;
    let mut total_warnings = 0;

    for path in file_paths {
        let result = scan_file(path);
        if !result.metadata_items.is_empty() {
            total_with_metadata += 1;
        }
        if !result.errors.is_empty() {
            total_errors += 1;
        }
        if !result.warnings.is_empty() {
            total_warnings += 1;
        }
        files.push(result);
    }

    BatchScanResult {
        files,
        total_scanned: file_paths.len(),
        total_with_metadata,
        total_errors,
        total_warnings,
    }
}

fn build_category_summary(items: &[MetadataItem]) -> Vec<CategorySummary> {
    let mut map: HashMap<MetadataCategory, CategorySummary> = HashMap::new();
    for item in items {
        let entry = map
            .entry(item.category.clone())
            .or_insert_with(|| CategorySummary {
                category: item.category.clone(),
                item_count: 0,
                removable_count: 0,
                partial_count: 0,
                read_only_count: 0,
                unsupported_count: 0,
            });
        entry.item_count += 1;
        match item.capability {
            RemovalCapability::Removable => entry.removable_count += 1,
            RemovalCapability::Partial => entry.partial_count += 1,
            RemovalCapability::ReadOnly => entry.read_only_count += 1,
            RemovalCapability::Unsupported => entry.unsupported_count += 1,
        }
    }
    let mut summaries: Vec<_> = map.into_values().collect();
    summaries.sort_by(|a, b| a.category.to_string().cmp(&b.category.to_string()));
    summaries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_file_type() {
        assert_eq!(detect_file_type(Path::new("test.jpg")), FileType::Image);
        assert_eq!(detect_file_type(Path::new("test.wav")), FileType::Audio);
        assert_eq!(detect_file_type(Path::new("test.avi")), FileType::Video);
        assert_eq!(detect_file_type(Path::new("test.mkv")), FileType::Video);
        assert_eq!(detect_file_type(Path::new("test.docx")), FileType::Document);
        assert_eq!(detect_file_type(Path::new("test.xyz")), FileType::Other);
    }

    #[test]
    fn test_is_supported_extension() {
        assert!(is_supported_extension(Path::new("test.wav")));
        assert!(is_supported_extension(Path::new("test.avi")));
        assert!(is_supported_extension(Path::new("test.mkv")));
        assert!(is_supported_extension(Path::new("test.webm")));
        assert!(!is_supported_extension(Path::new("test.xyz")));
    }
}
