use std::{
    cell::RefCell,
    ffi::c_int,
    marker::PhantomData,
    ptr::NonNull,
    rc::Rc,
    time::{Duration, Instant},
};

use libc::free;

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
}

impl Drop for Timeout {
    fn drop(&mut self) {
        if *self.registered.borrow() {
            self.delete(); // delete
        }
        // use libc::free timeout instance
        unsafe {
            let raw_ptr = self.raw.as_ptr() as *mut std::ffi::c_void;
            free(raw_ptr);
        }
    }
}

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

pub struct TimeoutSIt {
    raw: NonNull<timeouts_it>,
}

impl TimeoutSIt {
    /// flag has 4 value: PENDING, EXPIRED, ALL, CLEAR
    fn new(flags: TimeoutSItFlag) -> Result<TimeoutSIt, &'static str> {
        let mut instance = Box::into_raw(Box::new(timeouts_it::default()));
        TIMEOUTS_IT_INIT(instance, flags as i32);
        let raw = NonNull::new(instance).ok_or("Failed to create TimeoutSIt")?;
        Ok(TimeoutSIt { raw })
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
    pub fn get_next_wait_time(&mut self) -> timeout_t {
        let t = unsafe { timeouts_timeout(self.get_raw()) };
        // timeouts_timeout 取到一定程度会返回 u64 最大值
        if t == u64::MAX {
            0
        } else {
            t
        }
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
    }
}
