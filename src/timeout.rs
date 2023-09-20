use std::{
    cell::RefCell,
    ffi::c_int,
    marker::PhantomData,
    ptr::NonNull,
    rc::Rc,
    time::{Duration, Instant},
};

use libc::{c_void, free};

use crate::timeout_bind::*;

/// timeout flag: relative time or absolute time, default relative time
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimeoutType {
    INT = TIMEOUT_INT as isize,         // relative time
    ABS = TIMEOUT_ABS as isize,         // absolute time
    Default = TIMEOUT_DEFAULT as isize, // relative time
}
// as i32
impl From<TimeoutType> for i32 {
    fn from(timeout_type: TimeoutType) -> Self {
        timeout_type as i32
    }
}

impl TimeoutType {
    // i32 -> TimeoutType
    fn new(flag: i32) -> Self {
        match flag {
            TIMEOUT_INT => TimeoutType::INT,
            TIMEOUT_ABS => TimeoutType::ABS,
            _ => TimeoutType::Default,
        }
    }
}

type TimeoutCallBack = timeout_cb;

impl TimeoutCallBack {
    pub fn new<T>(callback: extern "C" fn(arg: *mut c_void), arg: *mut T) -> Self {
        let fn_ptr = Some(callback);
        TimeoutCallBack {
            fn_ptr,
            arg: arg as *mut c_void,
        }
    }

    pub fn call(&self) -> bool {
        if let Some(callback) = self.fn_ptr {
            callback(self.arg);
            return true;
        }
        return false;
    }
}

pub struct Timeout {
    // raw: Box<timeout>, // raw pointer
    raw: NonNull<timeout>,
    registered: Rc<RefCell<bool>>,
}

impl Timeout {
    pub fn new(flags: TimeoutType) -> Result<Timeout, &'static str> {
        // timeout instantiated in rust rather than C side
        // Box::into_raw let instance ownership transfer to C
        let raw = Box::into_raw(Box::new(timeout::default()));
        let raw = unsafe { timeout_init(raw, flags as i32) };
        let raw = NonNull::new(raw).ok_or("Failed to create Timeout")?;
        Ok(Timeout {
            raw,
            registered: Rc::new(RefCell::new(false)),
        })
    }
    // gen Timeout from raw pointer
    pub fn gen(to: NonNull<timeout>, registered: bool) -> Timeout {
        Timeout {
            raw: to,
            registered: Rc::new(RefCell::new(registered)),
        }
    }

    // get raw pointer
    pub fn get_raw(&self) -> *mut timeout {
        self.raw.as_ptr()
    }
    ///
    pub fn set_registered(&self, flag: bool) {
        *self.registered.borrow_mut() = flag;
    }

    pub fn set_cb(&self, cb: TimeoutCallBack) {
        unsafe {
            (*self.raw.as_ptr()).callback = cb;
        }
    }

    /// return true if timeout is registered and on timing wheel
    pub fn is_pending(&self) -> bool {
        self.registered.borrow().clone() && unsafe { timeout_pending(self.get_raw()) }
    }

    /// return true if timeout is registered and on expired queue
    pub fn is_expired(&self) -> bool {
        self.registered.borrow().clone() && unsafe { timeout_expired(self.get_raw()) }
    }

    /// remove timeout from any timing wheel or expired queue (okay if on neither)
    pub fn delete(&self) {
        unsafe { timeout_del(self.get_raw()) };
    }

    pub fn run_cb(&self) -> bool {
        let cb = unsafe { (*self.raw.as_ptr()).callback };
        return cb.call();
    }
}

impl Drop for Timeout {
    fn drop(&mut self) {
        if *self.registered.borrow() {
            self.delete(); // delete
        }
        // use libc::free timeout instance
        unsafe {
            let raw_ptr = self.raw.as_ptr() as *mut c_void;
            free(raw_ptr);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimeoutSItFlag {
    PENDING = TIMEOUTS_PENDING as isize,
    EXPIRED = TIMEOUTS_EXPIRED as isize,
    ALL = TIMEOUTS_ALL as isize,
    CLEAR = TIMEOUTS_CLEAR as isize,
}
// as i32
impl From<TimeoutSItFlag> for i32 {
    fn from(flag: TimeoutSItFlag) -> Self {
        flag as i32
    }
}

impl TimeoutSItFlag {
    pub fn new(flag: i32) -> Self {
        match flag {
            TIMEOUTS_PENDING => TimeoutSItFlag::PENDING,
            TIMEOUTS_EXPIRED => TimeoutSItFlag::EXPIRED,
            TIMEOUTS_ALL => TimeoutSItFlag::ALL,
            TIMEOUTS_CLEAR => TimeoutSItFlag::CLEAR,
            _ => TimeoutSItFlag::ALL,
        }
    }
}

pub struct TimeoutSIt {
    raw: NonNull<timeouts_it>,
}

impl TimeoutSIt {
    /// flag has 4 value: PENDING, EXPIRED, ALL, CLEAR
    fn new(flags: TimeoutSItFlag) -> Result<TimeoutSIt, &'static str> {
        let instance = Box::into_raw(Box::new(timeouts_it::default()));
        TIMEOUTS_IT_INIT(instance, flags as i32);
        let raw = NonNull::new(instance).ok_or("Failed to create TimeoutSIt")?;
        Ok(TimeoutSIt { raw })
    }
}

impl Drop for TimeoutSIt {
    fn drop(&mut self) {
        unsafe {
            let raw_ptr = self.raw.as_ptr() as *mut c_void;
            free(raw_ptr);
        }
    }
}

// TimeoutManager
pub struct TimeoutManager {
    tos: NonNull<timeouts>,
}

impl TimeoutManager {
    /// if hz_set = 0, default hz_set = TIMEOUT_mHZ
    pub fn new(hz_set: timeout_t) -> Result<TimeoutManager, &'static str> {
        let mut err = 0 as usize;
        // if hz_set = 0, default set to TIMEOUT_mHZ (timeouts_open)
        let tos = unsafe { timeouts_open(hz_set, &mut err) };
        if err != 0 {
            return Err("Failed to create timeout manager, null");
        }
        let tos = NonNull::new(tos).ok_or("Failed to create timeout manager, null")?;
        Ok(TimeoutManager { tos })
    }

    // get raw pointer
    fn get_raw(&self) -> *mut timeouts {
        self.tos.as_ptr()
    }

    // close
    pub fn close(&mut self) {
        unsafe {
            timeouts_close(self.get_raw());
        }
    }

    fn update_time(&mut self, time: timeout_t, timeout_type: TimeoutType) {
        let tmp = self.get_raw();
        match timeout_type {
            TimeoutType::INT => unsafe { timeouts_step(self.get_raw(), time) },
            TimeoutType::ABS => unsafe { timeouts_update(self.get_raw(), time) },
            TimeoutType::Default => unsafe { timeouts_step(self.get_raw(), time) },
        }
    }
    /// update time: relative time
    pub fn update_time_int(&mut self, time: timeout_t) {
        self.update_time(time, TimeoutType::INT);
    }
    /// update time: absolute time
    pub fn update_time_abs(&mut self, current_time: timeout_t) {
        self.update_time(current_time, TimeoutType::ABS);
    }
    /// get tos hz
    pub fn get_hz(&self) -> timeout_t {
        unsafe { timeouts_hz(self.get_raw()) }
    }
    /// return interval to next required update
    /// careful for two case:
    /// - return value could be u64::MAX, it means no timeout
    /// - return value will always be less than next timeout
    pub fn get_next_wait_time(&mut self) -> timeout_t {
        unsafe { timeouts_timeout(self.get_raw()) }
    }
    /// return true if any timeouts pending on timing wheel
    pub fn any_pending(&mut self) -> bool {
        unsafe { timeouts_pending(self.get_raw()) }
    }
    /// return true if any timeouts on expired queue
    pub fn any_expired(&mut self) -> bool {
        unsafe { timeouts_expired(self.get_raw()) }
    }
    // return true if TimeoutManager is effective
    pub fn check(&mut self) -> bool {
        unsafe { timeouts_check(self.get_raw(), stderr) }
    }
}

impl TimeoutManager {
    /// add timeout to timing wheel
    pub fn add(&mut self, to: &Timeout, time: timeout_t) {
        to.set_registered(true);
        unsafe { timeouts_add(self.get_raw(), to.get_raw(), time) };
    }
    /// remove timeout from any timing wheel or expired queue (okay if on neither)
    pub fn delete(&mut self, to: &Timeout) {
        unsafe { timeouts_del(self.get_raw(), to.get_raw()) };
    }
    /// return next expired timeout, or NULL if none
    pub fn next_expired_timeout(&mut self) -> Option<Timeout> {
        let to: *mut timeout = unsafe { timeouts_get(self.get_raw()) };
        if to.is_null() {
            return None;
        }
        let to = NonNull::new(to).unwrap();
        return Some(Timeout::gen(to, true));
    }
    /// return next timeout as timeout_sit requested, or NULL if none
    pub fn next_timeout(&mut self, timeout_sit: &TimeoutSIt) -> Option<Timeout> {
        let to: *mut timeout = unsafe { timeouts_next(self.get_raw(), timeout_sit.raw.as_ptr()) };
        if to.is_null() {
            return None;
        }
        let to = NonNull::new(to).unwrap();
        return Some(Timeout::gen(to, true));
    }
}

impl Drop for TimeoutManager {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeout_type() {
        let int_type = TimeoutType::INT;
        let abs_type = TimeoutType::ABS;
        let default_type = TimeoutType::Default;

        assert_eq!(i32::from(int_type), TIMEOUT_INT);
        assert_eq!(i32::from(abs_type), TIMEOUT_ABS);
        assert_eq!(i32::from(default_type), TIMEOUT_DEFAULT);

        assert_eq!(TimeoutType::new(TIMEOUT_INT), int_type);
        assert_eq!(TimeoutType::new(TIMEOUT_ABS), abs_type);
        assert_eq!(TimeoutType::new(123), default_type);
    }

    #[test]
    fn test_timeout() {
        let timeout = Timeout::new(TimeoutType::Default).unwrap(); // relative timeout
        assert!(!timeout.raw.as_ptr().is_null());
        assert!(!timeout.is_pending());
        assert!(!timeout.is_expired());

        let mut tos = TimeoutManager::new(TIMEOUT_mHZ).unwrap();
        tos.update_time_int(0); // tos.now = 0
        tos.add(&timeout, 100); // expired time = tos.now + 100
        assert!(timeout.is_pending());
        assert!(!timeout.is_expired());

        tos.update_time_int(98); // tos.now = 99
        assert!(timeout.is_pending());
        assert!(!timeout.is_expired());

        tos.update_time_int(10); // tos.now = 109
        assert!(!timeout.is_pending());
        assert!(timeout.is_expired());

        extern "C" fn rust_callback(arg: *mut c_void) {
            let value = unsafe { *(arg as *mut i32) };
            println!("Callback executed with arg: {}", value);
        }
        let arg: i32 = 42;
        let callback = TimeoutCallBack::new(rust_callback, &arg as *const _ as *mut c_void);
        timeout.set_cb(callback);
        assert!(timeout.run_cb());
    }

    #[test]
    fn test_timeout_sit_flag_into_i32() {
        let pending = TimeoutSItFlag::PENDING;
        let expired = TimeoutSItFlag::EXPIRED;
        let all = TimeoutSItFlag::ALL;
        let clear = TimeoutSItFlag::CLEAR;

        assert_eq!(i32::from(pending), TIMEOUTS_PENDING);
        assert_eq!(i32::from(expired), TIMEOUTS_EXPIRED);
        assert_eq!(i32::from(all), TIMEOUTS_ALL);
        assert_eq!(i32::from(clear), TIMEOUTS_CLEAR);

        assert_eq!(TimeoutSItFlag::new(TIMEOUTS_PENDING), pending);
        assert_eq!(TimeoutSItFlag::new(TIMEOUTS_EXPIRED), expired);
        assert_eq!(TimeoutSItFlag::new(TIMEOUTS_ALL), all);
        assert_eq!(TimeoutSItFlag::new(TIMEOUTS_CLEAR), clear);
        assert_eq!(TimeoutSItFlag::new(123), all);
    }

    #[test]
    fn test_timeout_sit_new() {
        let sit = TimeoutSIt::new(TimeoutSItFlag::PENDING);
        assert!(sit.is_ok());
    }

    #[test]
    fn test_timeout_manger() {
        let mut tos = TimeoutManager::new(TIMEOUT_mHZ).unwrap();
        assert_eq!(tos.get_hz(), TIMEOUT_mHZ);
        assert!(tos.check());

        tos.update_time_abs(0);
        assert_eq!(tos.get_next_wait_time(), u64::MAX); // no timeout wait, so wait time is u64::MAX

        let timeout = Timeout::new(TimeoutType::Default).unwrap(); // relative timeout
        tos.add(&timeout, 100); // expired time = tos.now + 100

        tos.update_time_abs(30);
        assert!(tos.any_pending());
        assert!(!tos.any_expired());
        assert!(tos.get_next_wait_time() < 70);

        tos.update_time_abs(100);
        assert!(!tos.any_pending());
        assert!(tos.any_expired());
        assert_eq!(tos.get_next_wait_time(), 0);
    }
}
