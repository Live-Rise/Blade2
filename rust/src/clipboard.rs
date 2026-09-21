//! 复制消息走的是 Win32 `CF_UNICODETEXT`：主干用 WinRT 的 `DataPackage` + `Clipboard.SetContent`，
//! 而 reactor 0.100 只重出了 `windows_time`，剪贴板类型拿不到，所以自己声明这几个入口。

use std::ffi::c_void;

const CF_UNICODETEXT: u32 = 13;
const GMEM_MOVEABLE: u32 = 0x0002;

#[link(name = "user32")]
unsafe extern "system" {
    fn OpenClipboard(owner: *mut c_void) -> i32;
    fn CloseClipboard() -> i32;
    fn EmptyClipboard() -> i32;
    fn SetClipboardData(format: u32, handle: *mut c_void) -> *mut c_void;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GlobalAlloc(flags: u32, bytes: usize) -> *mut c_void;
    fn GlobalLock(handle: *mut c_void) -> *mut c_void;
    fn GlobalUnlock(handle: *mut c_void) -> i32;
    fn GlobalFree(handle: *mut c_void) -> *mut c_void;
}

/// 成功返回 true；主干的失败口径同样是「什么都不做、也不换图标」。
pub fn copy_text(text: &str) -> bool {
    let utf16: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    let bytes = utf16.len() * 2;
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return false;
        }
        EmptyClipboard();
        let handle = GlobalAlloc(GMEM_MOVEABLE, bytes);
        if handle.is_null() {
            CloseClipboard();
            return false;
        }
        let dest = GlobalLock(handle);
        if dest.is_null() {
            GlobalFree(handle);
            CloseClipboard();
            return false;
        }
        std::ptr::copy_nonoverlapping(utf16.as_ptr().cast::<u8>(), dest.cast::<u8>(), bytes);
        GlobalUnlock(handle);
        let placed = !SetClipboardData(CF_UNICODETEXT, handle).is_null();
        if !placed {
            GlobalFree(handle);
        }
        CloseClipboard();
        placed
    }
}
