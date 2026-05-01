use core::slice;
use common::FramebufferInfo;

static mut FB_PTR: *mut u8 = core::ptr::null_mut();
static mut FB_W: usize = 0;
static mut FB_H: usize = 0;
static mut FB_PITCH: usize = 0;
static mut FB_BPP: usize = 0;

pub fn init(info: &FramebufferInfo) {
    unsafe {
        FB_PTR = info.addr as *mut u8;
        FB_W = info.width as usize;
        FB_H = info.height as usize;
        FB_PITCH = info.pitch as usize;
        FB_BPP = info.bpp as usize;
    }
}

pub unsafe fn raw_ptr() -> *mut u8 {
    FB_PTR
}

pub fn clear(color: u32) {
    unsafe {
        if FB_PTR.is_null() || FB_BPP == 0 { return; }
        let row = [color as u8; 4];
        for y in 0..FB_H {
            for x in 0..FB_W {
                let off = y * FB_PITCH + x * (FB_BPP / 8);
                slice::from_raw_parts_mut(FB_PTR.add(off), FB_BPP / 8).copy_from_slice(&row[..FB_BPP/8]);
            }
        }
    }
}

pub fn draw_pixel(x: usize, y: usize, color: u32) {
    unsafe {
        if FB_PTR.is_null() || FB_BPP == 0 || x >= FB_W || y >= FB_H { return; }
        let off = y * FB_PITCH + x * (FB_BPP / 8);
        let bytes = &mut *slice::from_raw_parts_mut(FB_PTR.add(off), FB_BPP / 8);
        let src = color.to_le_bytes();
        for i in 0..bytes.len() {
            bytes[i] = src[i];
        }
    }
}

pub fn draw_rect(x: usize, y: usize, w: usize, h: usize, color: u32) {
    for dy in 0..h {
        for dx in 0..w {
            draw_pixel(x + dx, y + dy, color);
        }
    }
}
