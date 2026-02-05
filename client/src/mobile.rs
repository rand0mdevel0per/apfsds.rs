//! Mobile FFI bindings
//!
//! Provides C-compatible entry points for iOS/Android.

use crate::config::ClientConfig;
use std::ffi::CStr;
use std::os::raw::c_char;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn apfsds_mobile_start(config_path: *const c_char) -> i32 {
    let _c_str = unsafe {
        if config_path.is_null() {
            return -1;
        }
        CStr::from_ptr(config_path)
    };

    // In a real app we would start the runtime here.
    // For now stub returns success.
    0
}
