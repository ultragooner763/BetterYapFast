//! Custom wallpaper management: loading, blur, dimming, collection, and Slint integration.

use std::{
    cell::RefCell,
    collections::HashMap,
    path::{Path, PathBuf},
};

use image::{imageops::FilterType, GenericImageView, RgbaImage};
use slint::{ComponentHandle, ModelRc, SharedPixelBuffer, VecModel};
use yapfast_ui::{Wallpaper, WallpaperTile, YapfastWindow};

thread_local! {
    static CACHED_BASE: RefCell<Option<(PathBuf, RgbaImage)>> = const { RefCell::new(None) };
    static WALLPAPERS: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
    static THUMBNAILS: RefCell<HashMap<PathBuf, slint::Image>> = RefCell::new(HashMap::new());
}

pub fn load_image(path: &Path) -> Result<RgbaImage, String> {
    let img = image::open(path).map_err(|e| e.to_string())?;
    let (w, h) = img.dimensions();
    // Pre-downscale if larger than 1920x1080 for responsive rendering and memory efficiency.
    let rgba = if w > 1920 || h > 1080 {
        let ratio = (1920.0 / w as f32).min(1080.0 / h as f32);
        let new_w = (w as f32 * ratio).round().max(1.0) as u32;
        let new_h = (h as f32 * ratio).round().max(1.0) as u32;
        image::imageops::resize(&img.to_rgba8(), new_w, new_h, FilterType::Triangle)
    } else {
        img.into_rgba8()
    };
    Ok(rgba)
}

pub fn make_thumbnail(img: &RgbaImage) -> slint::Image {
    let thumb = image::imageops::resize(img, 120, 72, FilterType::Triangle);
    let (w, h) = thumb.dimensions();
    let buffer = SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&thumb, w, h);
    slint::Image::from_rgba8(buffer)
}

pub fn process_wallpaper(base: &RgbaImage, sigma: f32, dim: f32) -> RgbaImage {
    let mut processed = if sigma > 0.5 {
        image::imageops::blur(base, sigma)
    } else {
        base.clone()
    };
    // Bake dimming directly into RGBA pixels so Slint draws a single texture without extra layers.
    let factor = (1.0 - dim.clamp(0.0, 0.95)).max(0.05);
    for pixel in processed.pixels_mut() {
        pixel[0] = (pixel[0] as f32 * factor) as u8;
        pixel[1] = (pixel[1] as f32 * factor) as u8;
        pixel[2] = (pixel[2] as f32 * factor) as u8;
        pixel[3] = 255;
    }
    processed
}

pub fn to_slint_image(img: &RgbaImage) -> slint::Image {
    let (w, h) = img.dimensions();
    let buffer = SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(img, w, h);
    slint::Image::from_rgba8(buffer)
}

fn refresh_tiles_model(window: &YapfastWindow, paths: &[PathBuf], active_path: Option<&Path>) {
    let mut tiles = Vec::new();
    let mut active_idx = -1i32;

    for (index, path) in paths.iter().enumerate() {
        if path.is_file() {
            let cached_thumb = THUMBNAILS.with(|cache| cache.borrow().get(path).cloned());
            let preview = match cached_thumb {
                Some(thumb) => thumb,
                None => {
                    if let Ok(rgba) = load_image(path) {
                        let thumb = make_thumbnail(&rgba);
                        THUMBNAILS.with(|cache| {
                            cache.borrow_mut().insert(path.clone(), thumb.clone());
                        });
                        thumb
                    } else {
                        slint::Image::default()
                    }
                }
            };
            let path_str = path.to_string_lossy().to_string();
            if let Some(active) = active_path {
                if active == path {
                    active_idx = index as i32;
                }
            }
            tiles.push(WallpaperTile {
                id: format!("wp_{index}").into(),
                path: path_str.into(),
                preview,
            });
        }
    }

    let global = window.global::<Wallpaper>();
    global.set_list(ModelRc::new(VecModel::from(tiles)));
    global.set_active_index(active_idx);
}

pub fn init(
    window: &YapfastWindow,
    enabled: bool,
    wallpaper_path: Option<&str>,
    blur: f32,
    dim: f32,
    saved_list: &[String],
) {
    let global = window.global::<Wallpaper>();
    global.set_enabled(enabled);
    global.set_blur(blur);
    global.set_dim(dim);

    let mut paths: Vec<PathBuf> = saved_list
        .iter()
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .collect();

    if let Some(p_str) = wallpaper_path.filter(|p| !p.is_empty()) {
        let p = PathBuf::from(p_str);
        if p.is_file() && !paths.contains(&p) {
            paths.insert(0, p);
        }
    }

    let active_path = wallpaper_path
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .or_else(|| paths.first().cloned());

    WALLPAPERS.with(|wps| {
        *wps.borrow_mut() = paths.clone();
    });

    if let Some(ref path) = active_path {
        if let Ok(rgba) = load_image(path) {
            let thumb = make_thumbnail(&rgba);
            THUMBNAILS.with(|cache| {
                cache.borrow_mut().insert(path.clone(), thumb);
            });
            let processed = process_wallpaper(&rgba, blur, dim);
            let slint_img = to_slint_image(&processed);
            CACHED_BASE.with(|cache| {
                *cache.borrow_mut() = Some((path.clone(), rgba));
            });
            global.set_path(path.to_string_lossy().as_ref().into());
            global.set_image(slint_img);
        }
    }

    refresh_tiles_model(window, &paths, active_path.as_deref());
}

pub fn register_callbacks(window: &YapfastWindow) {
    let weak = window.as_weak();
    let global = window.global::<Wallpaper>();

    // Enabled toggled callback
    let w = weak.clone();
    global.on_enabled_toggled(move |enabled| {
        let Some(window) = w.upgrade() else { return };
        window.global::<Wallpaper>().set_enabled(enabled);
        window.invoke_preferences_changed();
    });

    // Choose/Add wallpaper callback
    let w = weak.clone();
    global.on_choose(move || {
        let Some(window) = w.upgrade() else { return };
        let w2 = window.as_weak();
        let _ = slint::spawn_local(async move {
            let file = rfd::AsyncFileDialog::new()
                .set_title("Add Wallpaper")
                .add_filter("Images", &["png", "jpg", "jpeg", "webp", "bmp"])
                .pick_file()
                .await;

            if let (Some(file), Some(window)) = (file, w2.upgrade()) {
                let path = file.path().to_owned();
                let path_str = path.to_string_lossy().to_string();
                let global = window.global::<Wallpaper>();
                let blur = global.get_blur();
                let dim = global.get_dim();

                let path_clone = path.clone();
                let w3 = window.as_weak();
                std::thread::spawn(move || {
                    if let Ok(rgba) = load_image(&path_clone) {
                        let thumb = make_thumbnail(&rgba);
                        THUMBNAILS.with(|cache| {
                            cache.borrow_mut().insert(path_clone.clone(), thumb);
                        });
                        let processed = process_wallpaper(&rgba, blur, dim);
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(win) = w3.upgrade() {
                                let slint_img = to_slint_image(&processed);
                                CACHED_BASE.with(|cache| {
                                    *cache.borrow_mut() = Some((path_clone.clone(), rgba));
                                });
                                let g = win.global::<Wallpaper>();
                                g.set_path(path_str.as_str().into());
                                g.set_image(slint_img);
                                g.set_enabled(true);

                                WALLPAPERS.with(|wps| {
                                    let mut list = wps.borrow_mut();
                                    if !list.contains(&path_clone) {
                                        list.insert(0, path_clone.clone());
                                    }
                                    let paths = list.clone();
                                    drop(list);
                                    refresh_tiles_model(&win, &paths, Some(&path_clone));
                                });
                                win.invoke_preferences_changed();
                            }
                        });
                    }
                });
            }
        });
    });

    // Select wallpaper from collection
    let w = weak.clone();
    global.on_select(move |index| {
        let Some(window) = w.upgrade() else { return };
        let index = usize::try_from(index).ok();
        let path = index.and_then(|i| {
            WALLPAPERS.with(|wps| wps.borrow().get(i).cloned())
        });

        if let Some(path) = path {
            let path_clone = path.clone();
            let global = window.global::<Wallpaper>();
            let blur = global.get_blur();
            let dim = global.get_dim();
            let w2 = window.as_weak();

            std::thread::spawn(move || {
                if let Ok(rgba) = load_image(&path_clone) {
                    let processed = process_wallpaper(&rgba, blur, dim);
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(win) = w2.upgrade() {
                            let slint_img = to_slint_image(&processed);
                            CACHED_BASE.with(|cache| {
                                *cache.borrow_mut() = Some((path_clone.clone(), rgba));
                            });
                            let g = win.global::<Wallpaper>();
                            g.set_path(path_clone.to_string_lossy().as_ref().into());
                            g.set_image(slint_img);
                            g.set_enabled(true);

                            let all_paths = WALLPAPERS.with(|wps| wps.borrow().clone());
                            refresh_tiles_model(&win, &all_paths, Some(&path_clone));
                            win.invoke_preferences_changed();
                        }
                    });
                }
            });
        }
    });

    // Remove wallpaper from collection
    let w = weak.clone();
    global.on_remove(move |index| {
        let Some(window) = w.upgrade() else { return };
        let index = usize::try_from(index).ok();
        if let Some(i) = index {
            let removed_path = WALLPAPERS.with(|wps| {
                let mut list = wps.borrow_mut();
                if i < list.len() {
                    Some(list.remove(i))
                } else {
                    None
                }
            });

            if let Some(removed) = removed_path {
                let all_paths = WALLPAPERS.with(|wps| wps.borrow().clone());
                let g = window.global::<Wallpaper>();
                let current_path = PathBuf::from(g.get_path().as_str());

                if current_path == removed {
                    if let Some(next) = all_paths.first() {
                        let next_clone = next.clone();
                        let blur = g.get_blur();
                        let dim = g.get_dim();
                        let w2 = window.as_weak();
                        std::thread::spawn(move || {
                            if let Ok(rgba) = load_image(&next_clone) {
                                let processed = process_wallpaper(&rgba, blur, dim);
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(win) = w2.upgrade() {
                                        let slint_img = to_slint_image(&processed);
                                        CACHED_BASE.with(|cache| {
                                            *cache.borrow_mut() = Some((next_clone.clone(), rgba));
                                        });
                                        let g = win.global::<Wallpaper>();
                                        g.set_path(next_clone.to_string_lossy().as_ref().into());
                                        g.set_image(slint_img);
                                        let paths = WALLPAPERS.with(|wps| wps.borrow().clone());
                                        refresh_tiles_model(&win, &paths, Some(&next_clone));
                                        win.invoke_preferences_changed();
                                    }
                                });
                            }
                        });
                    } else {
                        CACHED_BASE.with(|cache| *cache.borrow_mut() = None);
                        g.set_enabled(false);
                        g.set_path("".into());
                        g.set_image(slint::Image::default());
                        refresh_tiles_model(&window, &[], None);
                        window.invoke_preferences_changed();
                    }
                } else {
                    refresh_tiles_model(&window, &all_paths, Some(&current_path));
                    window.invoke_preferences_changed();
                }
            }
        }
    });

    // Blur changed callback
    let w = weak.clone();
    global.on_blur_changed(move |sigma| {
        let Some(window) = w.upgrade() else { return };
        let dim = window.global::<Wallpaper>().get_dim();
        let cached = CACHED_BASE.with(|cache| {
            cache.borrow().as_ref().map(|(_, img)| img.clone())
        });
        if let Some(base) = cached {
            let w2 = window.as_weak();
            std::thread::spawn(move || {
                let processed = process_wallpaper(&base, sigma, dim);
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = w2.upgrade() {
                        let slint_img = to_slint_image(&processed);
                        win.global::<Wallpaper>().set_image(slint_img);
                    }
                });
            });
        }
    });

    // Dim changed callback
    let w = weak.clone();
    global.on_dim_changed(move |dim| {
        let Some(window) = w.upgrade() else { return };
        let blur = window.global::<Wallpaper>().get_blur();
        let cached = CACHED_BASE.with(|cache| {
            cache.borrow().as_ref().map(|(_, img)| img.clone())
        });
        if let Some(base) = cached {
            let w2 = window.as_weak();
            std::thread::spawn(move || {
                let processed = process_wallpaper(&base, blur, dim);
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = w2.upgrade() {
                        let slint_img = to_slint_image(&processed);
                        win.global::<Wallpaper>().set_image(slint_img);
                    }
                });
            });
        }
    });
}

pub fn get_saved_wallpapers() -> Vec<String> {
    WALLPAPERS.with(|wps| {
        wps.borrow()
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect()
    })
}
