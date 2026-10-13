//! Direct bindings to NASA NAIF CSPICE. All calls must hold the kernel-pool lock in sky.rs.
use std::ffi::{c_char, c_int, c_void};
#[repr(C)]
pub(crate) struct SpiceCell {
    pub dtype: c_int,
    pub length: c_int,
    pub size: c_int,
    pub card: c_int,
    pub is_set: c_int,
    pub adjust: c_int,
    pub init: c_int,
    pub base: *mut c_void,
    pub data: *mut c_void,
}
unsafe extern "C" {
    pub(crate) fn erract_c(operation: *const c_char, lenout: c_int, action: *mut c_char);
    pub(crate) fn errprt_c(operation: *const c_char, lenout: c_int, list: *mut c_char);
    pub(crate) fn failed_c() -> c_int;
    pub(crate) fn getmsg_c(option: *const c_char, lenout: c_int, msg: *mut c_char);
    pub(crate) fn reset_c();
    pub(crate) fn furnsh_c(path: *const c_char);
    pub(crate) fn unload_c(path: *const c_char);
    pub(crate) fn pckcov_c(path: *const c_char, id: c_int, coverage: *mut SpiceCell);
    pub(crate) fn wncard_c(window: *const SpiceCell) -> c_int;
    pub(crate) fn wnfetd_c(window: *const SpiceCell, index: c_int, left: *mut f64, right: *mut f64);
    pub(crate) fn spkez_c(
        target: c_int,
        et: f64,
        frame: *const c_char,
        abcorr: *const c_char,
        observer: c_int,
        state: *mut f64,
        lt: *mut f64,
    );
    pub(crate) fn pxform_c(from: *const c_char, to: *const c_char, et: f64, matrix: *mut [f64; 3]);
}
