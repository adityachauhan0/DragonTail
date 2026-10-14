use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Mutex;

use objc2::MainThreadMarker;
use objc2_app_kit::{NSEvent, NSEventMask};
use serde::Deserialize;

#[derive(Deserialize, Clone, Copy)]
pub struct CssRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

struct Mask {
    titlebar_height: f64,
    holes: Vec<CssRect>,
}

static MASK: Mutex<Mask> = Mutex::new(Mask {
    titlebar_height: 76.0,
    holes: Vec::new(),
});

static MAIN_NSWINDOW: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

const TRAFFIC_LIGHT_WIDTH: f64 = 78.0;

pub fn set_holes(height: f64, holes: Vec<CssRect>) {
    let Ok(mut mask) = MASK.lock() else {
        return;
    };
    if height > 0.0 {
        mask.titlebar_height = height;
    }
    mask.holes = holes;
}

pub fn install(main_window: *mut std::ffi::c_void) {
    MAIN_NSWINDOW.store(main_window, Ordering::SeqCst);
    let block = block2::RcBlock::new(|event: std::ptr::NonNull<NSEvent>| -> *mut NSEvent {
        let event_ref = unsafe { event.as_ref() };
        if should_drag(event_ref) {
            if let Some(mtm) = MainThreadMarker::new() {
                if let Some(window) = event_ref.window(mtm) {
                    window.performWindowDragWithEvent(event_ref);
                    return std::ptr::null_mut();
                }
            }
        }
        event.as_ptr()
    });
    let block = Box::leak(Box::new(block));
    let _monitor = unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::LeftMouseDown, block)
    };
    std::mem::forget(_monitor);
}

fn should_drag(event: &NSEvent) -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    let Some(window) = event.window(mtm) else {
        return false;
    };
    let main = MAIN_NSWINDOW.load(Ordering::SeqCst);
    if main.is_null() || !std::ptr::eq(objc2::rc::Retained::as_ptr(&window), main.cast()) {
        return false;
    }
    let Some(content) = window.contentView() else {
        return false;
    };
    let bounds = content.bounds();
    let view_height = bounds.size.height;
    let point = event.locationInWindow();
    let wx = point.x;
    let wy = point.y;
    let mask = MASK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let titlebar_bottom = view_height - mask.titlebar_height;
    if wy < titlebar_bottom || wx < TRAFFIC_LIGHT_WIDTH {
        return false;
    }
    for hole in &mask.holes {
        let left = hole.x;
        let right = hole.x + hole.width;
        let top = view_height - hole.y;
        let bottom = view_height - (hole.y + hole.height);
        if wx >= left && wx <= right && wy >= bottom && wy <= top {
            return false;
        }
    }
    true
}
