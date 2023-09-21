#![allow(non_upper_case_globals)] // allow snake case just in this file
use std::ptr::null_mut;

use libc::{c_char, c_int, c_uint, c_ulonglong, c_void, FILE};
// TODO 检查条件编译选项
// 目前暂时全部注释掉

/// V E R S I O N  I N T E R F A C E S

pub const TIMEOUT_VENDOR: &[u8; 26] = b"william@25thandClement.com";
pub const TIMEOUT_V_REL: u32 = 538313254;
pub const TIMEOUT_V_ABI: u32 = 538313252;
pub const TIMEOUT_V_API: u32 = 538313254;

extern "C" {
    pub fn timeout_version() -> c_int;
    pub fn timeout_vendor() -> *const c_char;
    pub fn timeout_v_rel() -> c_int;
    pub fn timeout_v_abi() -> c_int;
    pub fn timeout_v_api() -> c_int;
}

/// I N T E G E R  T Y P E  I N T E R F A C E S

// 原为宏定义, 使用静态变量代替 ~~需进一步检查~~
// test-timeout.c 测试通过
pub const TIMEOUT_PRIu: &[u8; 3] = b"lu\0"; // C 语言字符串结尾 `\0`, rust 没有
pub const TIMEOUT_PRIx: &[u8; 3] = b"lx\0";
pub const TIMEOUT_PRIX: &[u8; 3] = b"lX\0";

pub const TIMEOUT_mHZ: u64 = 1000;
pub const TIMEOUT_uHZ: u64 = 1000000;
pub const TIMEOUT_nHZ: u64 = 1000000000;

pub type timeout_t = u64;
pub type timeout_error_t = usize;

/// C A L L B A C K  I N T E R F A C E
///
/// Callback function parameters unspecified to make embedding into existing
/// applications easier.

// #[cfg(TIMEOUT_CB_OVERRIDE)]
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct timeout_cb {
    pub fn_ptr: Option<extern "C" fn(arg: *mut c_void)>,
    pub arg: *mut c_void,
}

/// T I M E O U T  I N T E R F A C E S

// #[cfg(TIMEOUT_DISABLE_INTERVALS)]
pub const TIMEOUT_DEFAULT: i32 = 0;
pub const TIMEOUT_INT: i32 = 1;
pub const TIMEOUT_ABS: i32 = 2;

// 宏定义 TIMEOUT_INITIALIZER timeout_setcb

// 来自 TAILQ_ENTRY 宏
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct timeout_TAILQ_ENTRY {
    pub tqe_next: *mut timeout,
    pub tqe_prev: *mut *mut timeout,
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct timeout {
    pub flags: c_int,
    pub expires: timeout_t,         /* absolute expiration time */
    pub pending: *mut timeout_list, /* timeout list if pending on wheel or expiry queue */
    pub tqe: timeout_TAILQ_ENTRY,   /* entry member for struct timeout_list lists */
    // #[cfg(TIMEOUT_DISABLE_CALLBACKS)]
    pub callback: timeout_cb, /* optional callback information */
    // #[cfg(TIMEOUT_DISABLE_INTERVALS)]
    pub interval: timeout_t, /* timeout interval if periodic */
    // #[cfg(TIMEOUT_DISABLE_RELATIVE_ACCESS)]
    pub timeouts: *mut timeouts, /* timeouts collection if member of */
}

impl Default for timeout {
    fn default() -> Self {
        timeout {
            flags: 0,
            expires: 0,
            pending: null_mut(),
            tqe: timeout_TAILQ_ENTRY {
                tqe_next: null_mut(),
                tqe_prev: null_mut(),
            },
            callback: timeout_cb {
                fn_ptr: None,
                arg: null_mut(),
            },
            interval: 0,
            timeouts: null_mut(),
        }
    }
}

extern "C" {
    /* initialize timeout structure (same as TIMEOUT_INITIALIZER) */
    pub fn timeout_init(arg1: *mut timeout, arg2: c_int) -> *mut timeout;
}
// #[cfg(TIMEOUT_DISABLE_RELATIVE_ACCESS)]

extern "C" {
    /* true if on timing wheel, false otherwise */
    pub fn timeout_pending(arg1: *mut timeout) -> bool;
    /* true if on expired queue, false otherwise */
    pub fn timeout_expired(arg1: *mut timeout) -> bool;
    /* remove timeout from any timing wheel (okay if not member of any) */
    pub fn timeout_del(arg1: *mut timeout);
}

/// T I M I N G  W H E E L  I N T E R F A C E S

// 原定义在 timeout.c line 206 | TAILQ_HEAD(timeout_list, timeout);
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct timeout_list {
    pub tqh_first: *mut timeout,
    pub tqh_last: *mut *mut timeout,
}

// for timeouts
const WHEEL_LEN: usize = 64; // timeout.c line 132 | (1U << WHEEL_BIT ) ,WHEEL_BIT = 6
const WHEEL_NUM: usize = 4;
pub type wheel_t = u64;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct timeouts {
    pub wheel: [[timeout_list; WHEEL_LEN]; WHEEL_NUM],
    pub expired: timeout_list,
    pub pending: [wheel_t; WHEEL_NUM],
    pub curtime: timeout_t,
    pub hertz: timeout_t,
}

extern "C" {
    /* open a new timing wheel, setting optional HZ (for float conversions) */
    pub fn timeouts_open(arg1: timeout_t, arg2: *mut timeout_error_t) -> *mut timeouts;
    /* destroy timing wheel */
    pub fn timeouts_close(arg1: *mut timeouts);
    /* return HZ setting (for float conversions) */
    pub fn timeouts_hz(arg1: *mut timeouts) -> timeout_t;
    /* update timing wheel with current absolute time */
    pub fn timeouts_update(arg1: *mut timeouts, arg2: timeout_t);
    /* step timing wheel by relative time */
    pub fn timeouts_step(arg1: *mut timeouts, arg2: timeout_t);
    /* return interval to next required update */
    pub fn timeouts_timeout(arg1: *mut timeouts) -> timeout_t;
    /* add timeout to timing wheel */
    pub fn timeouts_add(arg1: *mut timeouts, arg2: *mut timeout, arg3: timeout_t);
    /* remove timeout from any timing wheel or expired queue (okay if on neither) */
    pub fn timeouts_del(arg1: *mut timeouts, arg2: *mut timeout);
    /* return any expired timeout (caller should loop until NULL-return) */
    pub fn timeouts_get(arg1: *mut timeouts) -> *mut timeout;
    /* return true if any timeouts pending on timing wheel */
    pub fn timeouts_pending(arg1: *mut timeouts) -> bool;
    /* return true if any timeouts on expired queue */
    pub fn timeouts_expired(arg1: *mut timeouts) -> bool;
    /* return true if invariants hold. describes failures to optional file handle. */
    // arg2 use stderr is normal
    pub fn timeouts_check(arg1: *mut timeouts, arg2: *mut FILE) -> bool;
}

extern "C" {
    pub static mut stderr: *mut FILE;
}

pub const TIMEOUTS_PENDING: i32 = 16;
pub const TIMEOUTS_EXPIRED: i32 = 32;
pub const TIMEOUTS_ALL: i32 = 48;
pub const TIMEOUTS_CLEAR: i32 = 64;

// 原为 C 的宏定义 能转为 C 兼容函数
#[no_mangle]
pub extern "C" fn TIMEOUTS_IT_INITIALIZER(flags: c_int) -> timeouts_it {
    timeouts_it {
        flags: flags,
        pc: 0,
        i: 0,
        j: 0,
        to: null_mut(),
    }
}
// 原为 C 的宏定义 能转为 C 兼容函数
#[no_mangle]
pub extern "C" fn TIMEOUTS_IT_INIT(cur: *mut timeouts_it, flags: i32) {
    unsafe {
        (*cur).flags = flags;
        (*cur).pc = 0;
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct timeouts_it {
    pub flags: c_int,
    pub pc: c_uint,
    pub i: c_uint,
    pub j: c_uint,
    pub to: *mut timeout,
}

impl Default for timeouts_it {
    fn default() -> Self {
        timeouts_it {
            flags: 0,
            pc: 0,
            i: 0,
            j: 0,
            to: null_mut(),
        }
    }
}

/* return next timeout in pending wheel or expired queue. caller can delete
 * the returned timeout, but should not otherwise manipulate the timing
 * wheel. in particular, caller SHOULD NOT delete any other timeout as that
 * could invalidate cursor state and trigger a use-after-free.
 */
// #[link(name = "timeout")]
extern "C" {
    pub fn timeouts_next(arg1: *mut timeouts, arg2: *mut timeouts_it) -> *mut timeout;
}

/*
 * Calculate the interval our caller can wait before needing to process
 * events.
 */

//  extern "C" {
//      pub fn timeouts_int(T: *mut timeouts) -> timeout_t;
//  }
//  pub fn timeouts_timeout(t: *mut timeouts) -> timeout_t {
//      unsafe {
//          if !((*t).expired.tqh_first.is_null()) {
//              return 0;
//          }
//          timeouts_int(t)
//      }
//  }

// TODO: 宏定义 TIMEOUTS_FOREACH

// B O N U S  W H E E L  I N T E R F A C E S
// just use for lua, not use for rust.
