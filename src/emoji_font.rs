//! Registering a colour bitmap font, so emoji show inline in message text:
//! Slint's `StyledText` draws only glyphs. See [`crate::emoji_font_tables`] for
//! the font itself.

use std::sync::{Arc, Mutex};

use slint::fontique_012::{fontique, shared_collection};

pub use crate::emoji_font_tables::{SIDE, build};

static EMOJI_FAMILIES: Mutex<Option<(fontique::FamilyId, fontique::FamilyId)>> = Mutex::new(None);

/// Registers a font with each PNG of `glyphs`, which must be [`SIDE`] pixels
/// square, at `also`'s code points mapped to a glyph's, as `family`, and puts
/// it first in text's fallback chain after the UI typeface. True when the font
/// loaded.
pub fn register(family: &str, glyphs: &[(char, Vec<u8>)], also: &[(u32, char)]) -> bool {
    register_blob(
        family,
        fontique::Blob::new(Arc::new(build(family, glyphs, also))),
    )
}

/// Registers `font`, as [`register`] does one it builds.
pub fn register_blob(family: &str, font: fontique::Blob<u8>) -> bool {
    let Some(family_id) = register_family(family, font) else {
        return false;
    };
    let mut collection = shared_collection();
    for generic in [
        fontique::GenericFamily::SansSerif,
        fontique::GenericFamily::SystemUi,
    ] {
        let families: Vec<_> = collection.generic_families(generic).collect();
        collection.set_generic_families(generic, std::iter::once(family_id).chain(families));
    }
    true
}

/// Registers `font` under `family` name without altering the fallback chain directly.
pub fn register_family(family: &str, font: fontique::Blob<u8>) -> Option<fontique::FamilyId> {
    let mut collection = shared_collection();
    let registered = collection.register_fonts(
        font,
        Some(fontique::FontInfoOverride {
            family_name: Some(family),
            ..Default::default()
        }),
    );
    registered.first().map(|&(family_id, _)| family_id)
}

/// Registers both Twemoji and Apple emoji fonts and sets the fallback order.
pub fn register_emojis() {
    let tw_font = fontique::Blob::new(Arc::new(crate::twemoji::FONT));
    let apple_font = fontique::Blob::new(Arc::new(crate::twemoji::APPLE_FONT));

    let Some(tw_id) = register_family("Yapfast Twemoji", tw_font) else {
        tracing::warn!("could not register a font of Twemoji");
        return;
    };
    let Some(apple_id) = register_family("Yapfast Apple Emoji", apple_font) else {
        tracing::warn!("could not register a font of Apple Emoji");
        return;
    };

    *EMOJI_FAMILIES.lock().unwrap() = Some((tw_id, apple_id));
    set_fallback_order(crate::twemoji::using_apple());
}

/// Sets font fallback order: if `apple` is true, Apple Emoji is primary; otherwise Twemoji is primary.
pub fn set_fallback_order(apple: bool) {
    let guard = EMOJI_FAMILIES.lock().unwrap();
    let Some(&(tw_id, apple_id)) = guard.as_ref() else {
        return;
    };
    let primary = if apple { apple_id } else { tw_id };
    let secondary = if apple { tw_id } else { apple_id };

    let mut collection = shared_collection();
    for generic in [
        fontique::GenericFamily::SansSerif,
        fontique::GenericFamily::SystemUi,
    ] {
        let families: Vec<_> = collection
            .generic_families(generic)
            .filter(|&id| id != primary && id != secondary)
            .collect();
        collection.set_generic_families(
            generic,
            std::iter::once(primary)
                .chain(std::iter::once(secondary))
                .chain(families),
        );
    }
}
