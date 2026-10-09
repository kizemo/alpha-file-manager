//! File dialog detector — wraps `uia_event::get_dialog_info_if_match`.

use windows::Win32::Foundation::HWND;

/// Returns true if `hwnd` is a Win32 #32770 dialog with structural file-dialog features.
///
/// `isize` form is convenient for raw HWND values; converts to `HWND` internally.
pub fn is_file_dialog(hwnd_isize: isize) -> bool {
    if hwnd_isize == 0 {
        return false;
    }
    let hwnd = HWND(hwnd_isize as *mut core::ffi::c_void);
    crate::uia_event::get_dialog_info_if_match(hwnd).is_some()
}