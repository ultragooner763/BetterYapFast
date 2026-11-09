use std::{cell::RefCell, rc::Rc};
use slint::ComponentHandle;
use yapfast_ui::{ClientStyle, YapfastWindow};
use super::Adapter;

pub fn register_callbacks(window: &YapfastWindow, adapter: &Rc<RefCell<Adapter>>) {
    let style = window.global::<ClientStyle>();

    let weak = window.as_weak();
    let state = adapter.clone();
    style.on_select_font_preset(move |_index, font_name| {
        if let Some(window) = weak.upgrade() {
            let style = window.global::<ClientStyle>();
            style.set_native_font(font_name.clone());
            style.set_custom_font_path("".into());
            if let Ok(mut adapter) = state.try_borrow_mut() {
                adapter.layout.custom_font = Some(font_name.as_str().to_string());
                adapter.layout.custom_font_path = None;
                adapter.layout.write(&adapter.root);
            }
            crate::emoji_font::set_fallback_order(crate::twemoji::using_apple());
        }
    });

    let weak = window.as_weak();
    let state = adapter.clone();
    style.on_set_font_size(move |size| {
        if let Some(window) = weak.upgrade() {
            let style = window.global::<ClientStyle>();
            style.set_font_size_custom(size);
            if let Ok(mut adapter) = state.try_borrow_mut() {
                adapter.layout.font_size = Some(size);
                adapter.layout.write(&adapter.root);
            }
        }
    });

    let weak = window.as_weak();
    let state = adapter.clone();
    style.on_reset_font(move || {
        if let Some(window) = weak.upgrade() {
            let style = window.global::<ClientStyle>();
            style.set_native_font("DM Sans".into());
            style.set_custom_font_path("".into());
            style.set_font_size_custom(15.0);
            if let Ok(mut adapter) = state.try_borrow_mut() {
                adapter.layout.custom_font = Some("DM Sans".to_string());
                adapter.layout.custom_font_path = None;
                adapter.layout.font_size = Some(15.0);
                adapter.layout.write(&adapter.root);
            }
            crate::emoji_font::set_fallback_order(crate::twemoji::using_apple());
        }
    });

    let weak = window.as_weak();
    let state = adapter.clone();
    style.on_choose_custom_font(move || {
        let weak = weak.clone();
        let state = state.clone();
        let _ = slint::spawn_local(async move {
            let file = rfd::AsyncFileDialog::new()
                .set_title("Choose Custom Font (BetterYapFast)")
                .add_filter("Fonts (*.ttf, *.otf, *.woff, *.ttc)", &["ttf", "otf", "woff", "ttc"])
                .pick_file()
                .await;
            if let Some(file) = file {
                let path = file.path().to_path_buf();
                match crate::custom_font::load_and_register_font(&path) {
                    Ok(family_name) => {
                        if let Some(window) = weak.upgrade() {
                            let style = window.global::<ClientStyle>();
                            style.set_native_font(family_name.clone().into());
                            style.set_custom_font_path(path.to_string_lossy().to_string().into());
                            if let Ok(mut adapter) = state.try_borrow_mut() {
                                adapter.layout.custom_font = Some(family_name);
                                adapter.layout.custom_font_path = Some(path.to_string_lossy().to_string());
                                adapter.layout.write(&adapter.root);
                            }
                            crate::emoji_font::set_fallback_order(crate::twemoji::using_apple());
                        }
                    }
                    Err(err) => {
                        tracing::warn!("Failed to load font from {}: {}", path.display(), err);
                    }
                }
            }
        });
    });
}
