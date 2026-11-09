//! Custom fonts and typography engine for BetterYapFast.
//! Supports TTF, OTF, TTC, and WOFF 1.0 font loading at runtime,
//! with seamless fallback to Apple Color Emoji / Twemoji.

use std::{
    fs,
    io::Read,
    path::Path,
    sync::Arc,
};

use slint::fontique_012::{fontique, shared_collection};

pub const PRESET_FONTS: &[(&str, &str)] = &[
    ("DM Sans", "Modern geometric sans (Default)"),
    ("Atkinson Hyperlegible Next", "Balanced high-legibility typeface"),
    ("Satoshi", "Swiss neogrotesque aesthetic"),
    ("Inter", "Standard clean tech font"),
    ("Geist", "Vercel sleek developer aesthetic"),
    ("JetBrains Mono", "Iconic monospace coding font"),
    ("Segoe UI", "Native Windows typography"),
    ("Comic Sans MS", "Playful casual font"),
];

/// Decompresses a WOFF 1.0 font file into standard SFNT (TTF/OTF) if needed.
fn decompress_woff(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 44 || &data[0..4] != b"wOFF" {
        return None;
    }

    let flavor = u32::from_be_bytes(data[4..8].try_into().ok()?);
    let total_sfnt_size = u32::from_be_bytes(data[16..20].try_into().ok()?) as usize;
    let num_tables = u16::from_be_bytes(data[12..14].try_into().ok()?) as usize;

    let mut out = Vec::with_capacity(total_sfnt_size);
    // Write sfnt header
    out.extend_from_slice(&flavor.to_be_bytes());
    out.extend_from_slice(&(num_tables as u16).to_be_bytes());

    let entry_selector = (num_tables as f64).log2().floor() as u16;
    let search_range = (1 << entry_selector) * 16;
    let range_shift = (num_tables as u16) * 16 - search_range;

    out.extend_from_slice(&search_range.to_be_bytes());
    out.extend_from_slice(&entry_selector.to_be_bytes());
    out.extend_from_slice(&range_shift.to_be_bytes());

    // Directory size
    let header_size = 12 + num_tables * 16;
    let mut table_offset = header_size;
    let mut woff_offset = 44;

    let mut decompressed_tables = Vec::with_capacity(num_tables);

    for _ in 0..num_tables {
        if woff_offset + 20 > data.len() {
            return None;
        }
        let tag = &data[woff_offset..woff_offset + 4];
        let offset = u32::from_be_bytes(data[woff_offset + 4..woff_offset + 8].try_into().ok()?) as usize;
        let comp_len = u32::from_be_bytes(data[woff_offset + 8..woff_offset + 12].try_into().ok()?) as usize;
        let orig_len = u32::from_be_bytes(data[woff_offset + 12..woff_offset + 16].try_into().ok()?) as usize;
        let checksum = u32::from_be_bytes(data[woff_offset + 16..woff_offset + 20].try_into().ok()?);

        if offset + comp_len > data.len() {
            return None;
        }

        let raw_table = &data[offset..offset + comp_len];
        let table_bytes = if comp_len < orig_len {
            let mut decoder = flate2::read::ZlibDecoder::new(raw_table);
            let mut decompressed = Vec::with_capacity(orig_len);
            decoder.read_to_end(&mut decompressed).ok()?;
            decompressed
        } else {
            raw_table.to_vec()
        };

        // Align to 4 bytes
        let padded_len = (table_bytes.len() + 3) & !3;

        out.extend_from_slice(tag);
        out.extend_from_slice(&checksum.to_be_bytes());
        out.extend_from_slice(&(table_offset as u32).to_be_bytes());
        out.extend_from_slice(&(table_bytes.len() as u32).to_be_bytes());

        table_offset += padded_len;
        decompressed_tables.push((table_bytes, padded_len));
        woff_offset += 20;
    }

    for (mut table, padded_len) in decompressed_tables {
        table.resize(padded_len, 0);
        out.extend_from_slice(&table);
    }

    Some(out)
}

/// Loads a font from `path` (TTF, OTF, TTC, or WOFF), registers it into Slint's
/// shared font collection, and returns its family name.
pub fn load_and_register_font(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("Could not read font file: {e}"))?;

    // Check if WOFF
    let font_bytes = if &bytes[0..4.min(bytes.len())] == b"wOFF" {
        decompress_woff(&bytes).ok_or_else(|| "Failed to decompress WOFF font".to_string())?
    } else {
        bytes
    };

    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Custom Font")
        .to_string();

    let blob = fontique::Blob::new(Arc::new(font_bytes));
    let mut collection = shared_collection();

    // Register with fallback to file stem if internal family name is not detected
    let registered = collection.register_fonts(
        blob,
        Some(fontique::FontInfoOverride {
            family_name: Some(&stem),
            ..Default::default()
        }),
    );

    let (family_id, _) = registered
        .first()
        .ok_or_else(|| "Font file could not be parsed by fontique".to_string())?;

    let family_name = collection
        .family(*family_id)
        .map(|f| f.name().to_string())
        .unwrap_or(stem);

    // Ensure Apple Emoji fallback stays active in the generic chain
    crate::emoji_font::set_fallback_order(crate::twemoji::using_apple());

    Ok(family_name)
}
