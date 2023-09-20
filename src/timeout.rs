use std::{
    ffi::c_int,
    ptr::NonNull,
    time::{Duration, Instant},
};

use crate::timeout_bind::*;

/// timeout flag: 使用相对时间 or 绝对时间
pub enum TimeoutType {
    INT,     // 相对时间
    ABS,     // 绝对时间
    Default, // 相对时间
}
// 实现 as i32
impl From<TimeoutType> for i32 {
    fn from(timeout_type: TimeoutType) -> Self {
        match timeout_type {
            TimeoutType::INT => TIMEOUT_INT,
            TimeoutType::ABS => TIMEOUT_ABS,
            TimeoutType::Default => TIMEOUT_DEFAULT,
        }
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
    raw: NonNull<*mut timeout>, // timeout 是裸指针而不是实例, c 负责释放实例
}

impl Timeout {
    pub fn new(flags: TimeoutType) -> Result<Timeout, &'static str> {
        let mut instance = timeout::default();
        let raw = unsafe { timeout_init(&mut instance, flags as i32) };
        let raw = NonNull::new(raw as *mut _).ok_or("Failed to create timeout")?;
        Ok(Timeout { raw })
    }
    pub fn gen(to: NonNull<*mut timeout>) -> Timeout {
        Timeout { raw: to }
    }

    // 检查是否在 timing wheel 上
    pub fn is_pending(&self) -> bool {
        unsafe { timeout_pending(self.raw.as_ref().clone()) }
    }

    // 检查是否在 expired 队列上
    pub fn is_expired(&self) -> bool {
        unsafe { timeout_expired(self.raw.as_ref().clone()) }
    }

    // 从 timing wheel 上移除
    pub fn delete(&self) {
        unsafe { timeout_del(self.raw.as_ref().clone()) };
    }
    pub fn get_type(&self) -> TimeoutType {
        let flags = unsafe { (*(*self.raw.as_ptr())).flags };
        return TimeoutType::new(flags);
    }
}

impl Drop for Timeout {
    fn drop(&mut self) {
        self.delete(); // drop 时候先 delete
    }
}

pub enum TimeoutSItFlag {
    PENDING,
    EXPIRED,
    ALL,
    CLEAR,
}
// 实现 as i32
impl From<TimeoutSItFlag> for i32 {
    fn from(flag: TimeoutSItFlag) -> Self {
        match flag {
            TimeoutSItFlag::PENDING => TIMEOUTS_PENDING,
            TimeoutSItFlag::EXPIRED => TIMEOUTS_EXPIRED,
            TimeoutSItFlag::ALL => TIMEOUTS_ALL,
            TimeoutSItFlag::CLEAR => TIMEOUTS_CLEAR,
        }
    }
}

pub struct TimeoutSIt {
    raw: NonNull<*mut timeouts_it>,
}

impl TimeoutSIt {
    /// flag has 4 value: PENDING, EXPIRED, ALL, CLEAR
    fn new(flags: TimeoutSItFlag) -> Result<TimeoutSIt, &'static str> {
        let mut instance = timeouts_it::default();
        TIMEOUTS_IT_INIT(&mut instance, flags as i32);
        let raw = NonNull::new(&mut instance as *mut timeouts_it as *mut *mut timeouts_it)
            .ok_or("Failed to create TimeoutSIt")?;
        Ok(TimeoutSIt { raw })
    }
}

// TimeoutManager
pub struct TimeoutManager {
    tos: NonNull<*mut timeouts>,
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
        let tos = NonNull::new(tos as *mut _).ok_or("Failed to create timeout manager, null")?;
        Ok(TimeoutManager { tos })
    }

    // close
    pub fn close(&mut self) {
        unsafe {
            timeouts_close(self.tos.as_ref().clone());
        }
    }

    fn update_time(&mut self, time: timeout_t, timeout_type: TimeoutType) {
        match timeout_type {
            TimeoutType::INT => unsafe { timeouts_step(self.tos.as_ref().clone(), time) },
            TimeoutType::ABS => unsafe { timeouts_update(self.tos.as_ref().clone(), time) },
            TimeoutType::Default => unsafe { timeouts_step(self.tos.as_ref().clone(), time) },
        }
    }
    /// update time: relative time
    pub fn update_time_int(&mut self, current_time: timeout_t) {
        self.update_time(current_time, TimeoutType::INT);
    }
    /// update time: absolute time
    pub fn update_time_abs(&mut self, time: timeout_t) {
        self.update_time(time, TimeoutType::ABS);
    }
    /// get tos hz
    pub fn get_hz(&self) -> timeout_t {
        unsafe { timeouts_hz(self.tos.as_ref().clone()) }
    }
    /// return interval to next required update
    pub fn get_next_wait_time(&mut self) -> timeout_t {
        unsafe { timeouts_timeout(self.tos.as_ref().clone()) }
    }
    /// return true if any timeouts pending on timing wheel
    pub fn any_pending(&mut self) -> bool {
        unsafe { timeouts_pending(self.tos.as_ref().clone()) }
    }
    /// return true if any timeouts on expired queue
    pub fn any_expired(&mut self) -> bool {
        unsafe { timeouts_expired(self.tos.as_ref().clone()) }
    }
    // return true if TimeoutManager is effective
    pub fn check(&mut self) -> bool {
        unsafe { timeouts_check(self.tos.as_ref().clone(), stderr) }
    }
}

impl TimeoutManager {
    /// add timeout to timing wheel
    pub fn add(&mut self, to: Timeout, time: timeout_t) {
        unsafe { timeouts_add(self.tos.as_ref().clone(), to.raw.as_ref().clone(), time) };
    }
    /// remove timeout from any timing wheel or expired queue (okay if on neither)
    pub fn delete(&mut self, to: Timeout) {
        unsafe { timeouts_del(self.tos.as_ref().clone(), to.raw.as_ref().clone()) };
    }
    /// return next expired timeout, or NULL if none
    pub fn next_expired_timeout(&mut self) -> Option<Timeout> {
        let to: *mut timeout = unsafe { timeouts_get(self.tos.as_ref().clone()) };
        if to.is_null() {
            return None;
        }
        let to = NonNull::new(to as *mut *mut timeout).unwrap();
        return Some(Timeout::gen(to));
    }
    /// return next timeout as timeout_sit requested, or NULL if none
    pub fn next_timeout(&mut self, timeout_sit: TimeoutSIt) -> Option<Timeout> {
        let to: *mut timeout =
            unsafe { timeouts_next(self.tos.as_ref().clone(), timeout_sit.raw.as_ref().clone()) };
        if to.is_null() {
            return None;
        }
        let to = NonNull::new(to as *mut *mut timeout).unwrap();
        return Some(Timeout::gen(to));
    }
}

impl Drop for TimeoutManager {
    fn drop(&mut self) {
        self.close();
    }
}
