use std::{
    mem::{align_of, size_of, MaybeUninit},
    ptr::addr_of,
};

use crate::timeout::*;
use std::ffi::CStr;

#[cfg(test)]
// V E R S I O N  I N T E R F A C E S
#[test]
fn test_timeout_version() {
    let version = unsafe { timeout_version() };
    assert_eq!(version, TIMEOUT_V_REL as i32);
}

#[test]
fn test_timeout_vendor() {
    let vendor = unsafe { CStr::from_ptr(timeout_vendor()).to_bytes() };
    assert_eq!(vendor, TIMEOUT_VENDOR);
}

#[test]
fn test_timeout_v_rel() {
    let v_rel = unsafe { timeout_v_rel() };
    assert_eq!(v_rel, TIMEOUT_V_REL as i32);
}

#[test]
fn test_timeout_v_abi() {
    let v_abi = unsafe { timeout_v_abi() };
    assert_eq!(v_abi, TIMEOUT_V_ABI as i32);
}

#[test]
fn test_timeout_v_api() {
    let v_api = unsafe { timeout_v_api() };
    assert_eq!(v_api, TIMEOUT_V_API as i32);
}

// C A L L B A C K  I N T E R F A C E

#[test]
fn bindgen_test_layout_timeout_cb() {
    const UNINIT: MaybeUninit<timeout_cb> = MaybeUninit::uninit();
    let ptr = UNINIT.as_ptr();
    assert_eq!(
        size_of::<timeout_cb>(),
        16usize,
        concat!("Size of: ", stringify!(timeout_cb))
    );
    assert_eq!(
        align_of::<timeout_cb>(),
        8usize,
        concat!("Alignment of ", stringify!(timeout_cb))
    );
    assert_eq!(
        unsafe { addr_of!((*ptr).fn_) as usize - ptr as usize },
        0usize,
        concat!(
            "Offset of field: ",
            stringify!(timeout_cb),
            "::",
            stringify!(fn_)
        )
    );
    assert_eq!(
        unsafe { addr_of!((*ptr).arg) as usize - ptr as usize },
        8usize,
        concat!(
            "Offset of field: ",
            stringify!(timeout_cb),
            "::",
            stringify!(arg)
        )
    );
}

/// T I M E O U T  I N T E R F A C E S

#[test]
fn bindgen_test_layout_timeout_TAILQ_ENTRY() {
    const UNINIT: MaybeUninit<timeout_TAILQ_ENTRY> = MaybeUninit::uninit();
    let ptr = UNINIT.as_ptr();
    assert_eq!(
        size_of::<timeout_TAILQ_ENTRY>(),
        16usize,
        concat!("Size of: ", stringify!(timeout__bindgen_ty_1))
    );
    assert_eq!(
        align_of::<timeout_TAILQ_ENTRY>(),
        8usize,
        concat!("Alignment of ", stringify!(timeout__bindgen_ty_1))
    );
    assert_eq!(
        unsafe { addr_of!((*ptr).tqe_next) as usize - ptr as usize },
        0usize,
        concat!(
            "Offset of field: ",
            stringify!(timeout__bindgen_ty_1),
            "::",
            stringify!(tqe_next)
        )
    );
    assert_eq!(
        unsafe { addr_of!((*ptr).tqe_prev) as usize - ptr as usize },
        8usize,
        concat!(
            "Offset of field: ",
            stringify!(timeout__bindgen_ty_1),
            "::",
            stringify!(tqe_prev)
        )
    );
}
#[test]
fn bindgen_test_layout_timeout() {
    const UNINIT: MaybeUninit<timeout> = MaybeUninit::uninit();
    let ptr = UNINIT.as_ptr();
    assert_eq!(
        size_of::<timeout>(),
        72usize,
        concat!("Size of: ", stringify!(timeout))
    );
    assert_eq!(
        align_of::<timeout>(),
        8usize,
        concat!("Alignment of ", stringify!(timeout))
    );
    assert_eq!(
        unsafe { addr_of!((*ptr).flags) as usize - ptr as usize },
        0usize,
        concat!(
            "Offset of field: ",
            stringify!(timeout),
            "::",
            stringify!(flags)
        )
    );
    assert_eq!(
        unsafe { addr_of!((*ptr).expires) as usize - ptr as usize },
        8usize,
        concat!(
            "Offset of field: ",
            stringify!(timeout),
            "::",
            stringify!(expires)
        )
    );
    assert_eq!(
        unsafe { addr_of!((*ptr).pending) as usize - ptr as usize },
        16usize,
        concat!(
            "Offset of field: ",
            stringify!(timeout),
            "::",
            stringify!(pending)
        )
    );
    assert_eq!(
        unsafe { addr_of!((*ptr).tqe) as usize - ptr as usize },
        24usize,
        concat!(
            "Offset of field: ",
            stringify!(timeout),
            "::",
            stringify!(tqe)
        )
    );
    assert_eq!(
        unsafe { addr_of!((*ptr).callback) as usize - ptr as usize },
        40usize,
        concat!(
            "Offset of field: ",
            stringify!(timeout),
            "::",
            stringify!(callback)
        )
    );
    assert_eq!(
        unsafe { addr_of!((*ptr).interval) as usize - ptr as usize },
        56usize,
        concat!(
            "Offset of field: ",
            stringify!(timeout),
            "::",
            stringify!(interval)
        )
    );
    assert_eq!(
        unsafe { addr_of!((*ptr).timeouts) as usize - ptr as usize },
        64usize,
        concat!(
            "Offset of field: ",
            stringify!(timeout),
            "::",
            stringify!(timeouts)
        )
    );
}

#[test]
fn bindgen_test_layout_timeouts_it() {
    const UNINIT: MaybeUninit<timeouts_it> = MaybeUninit::uninit();
    let ptr = UNINIT.as_ptr();
    assert_eq!(
        size_of::<timeouts_it>(),
        24usize,
        concat!("Size of: ", stringify!(timeouts_it))
    );
    assert_eq!(
        align_of::<timeouts_it>(),
        8usize,
        concat!("Alignment of ", stringify!(timeouts_it))
    );
    assert_eq!(
        unsafe { ::std::ptr::addr_of!((*ptr).flags) as usize - ptr as usize },
        0usize,
        concat!(
            "Offset of field: ",
            stringify!(timeouts_it),
            "::",
            stringify!(flags)
        )
    );
    assert_eq!(
        unsafe { ::std::ptr::addr_of!((*ptr).pc) as usize - ptr as usize },
        4usize,
        concat!(
            "Offset of field: ",
            stringify!(timeouts_it),
            "::",
            stringify!(pc)
        )
    );
    assert_eq!(
        unsafe { ::std::ptr::addr_of!((*ptr).i) as usize - ptr as usize },
        8usize,
        concat!(
            "Offset of field: ",
            stringify!(timeouts_it),
            "::",
            stringify!(i)
        )
    );
    assert_eq!(
        unsafe { ::std::ptr::addr_of!((*ptr).j) as usize - ptr as usize },
        12usize,
        concat!(
            "Offset of field: ",
            stringify!(timeouts_it),
            "::",
            stringify!(j)
        )
    );
    assert_eq!(
        unsafe { ::std::ptr::addr_of!((*ptr).to) as usize - ptr as usize },
        16usize,
        concat!(
            "Offset of field: ",
            stringify!(timeouts_it),
            "::",
            stringify!(to)
        )
    );
}
