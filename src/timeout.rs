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

/// timeout type:
/// - INT: Periodic timeout, start time only supports relative time.
/// - ABS: Executed only once, start time is absolute time.
/// - Default: Executed only once, start time is relative time
/// TimeoutType could use as i32
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimeoutType {
    INT = TIMEOUT_INT as isize,         // periodic timeout, relative time
    ABS = TIMEOUT_ABS as isize,         // onetime timeout, absolute time
    Default = TIMEOUT_DEFAULT as isize, // onetime timeout, relative time
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

type TimeoutCallBack = timeout_cb; // callback

impl TimeoutCallBack {
    pub fn new<T>(callback: extern "C" fn(arg: *mut c_void), arg: *mut T) -> Self {
        let fn_ptr = Some(callback);
        TimeoutCallBack {
            fn_ptr,
            arg: arg as *mut c_void,
        }
    }
    /// run callback, if callback is None, return false
    pub fn call(&self) -> bool {
        if let Some(callback) = self.fn_ptr {
            callback(self.arg);
            return true;
        }
        return false;
    }
}

type Timeout = timeout;

impl Timeout {
    pub fn new(flags: TimeoutType) -> Result<Timeout, &'static str> {
        // timeout instantiated in rust rather than C side
        let raw = Box::into_raw(Box::new(timeout::default()));
        // Box::into_raw let instance ownership transfer to C
        let raw = unsafe { timeout_init(raw, flags as i32) };
        if raw.is_null() {
            return Err("Failed to create Timeout");
        }
        // Box::from_raw: instance ownership from C side to rust.
        Ok(*unsafe { Box::from_raw(raw) })
    }
    /// transfer *timeout to Timeout
    /// instance ownership form C side to rust
    pub fn transfer(to: *mut timeout) -> Timeout {
        *unsafe { Box::from_raw(to) }
    }
    /// set callback
    pub fn set_cb(&mut self, cb: TimeoutCallBack) {
        self.callback = cb;
    }

    /// return true if timeout is registered and on timing wheel
    pub fn is_pending(&self) -> bool {
        //C code: return to->pending && to->pending != &to->timeouts->expired;
        if !self.pending.is_null() // pending not null
            && !self.timeouts.is_null() // timeouts not null
            && self.pending != (unsafe { &mut (*self.timeouts).expired // pending not expired
            })
        {
            return true;
        }
        false
    }

    /// return true if timeout is registered and on expired queue
    pub fn is_expired(&self) -> bool {
        //return to->pending && to->pending == &to->timeouts->expired;
        if !self.pending.is_null() // pending not null
        && !self.timeouts.is_null() // timeouts not null
        && self.pending == (unsafe { &mut (*self.timeouts).expired // pending is expired
        }) {
            return true;
        }
        false
    }
    /// return true if timeout is periodic
    pub fn is_periodic(&self) -> bool {
        // if ((to->flags & TIMEOUT_INT) && to->interval > 0)
        return (self.flags & TIMEOUT_INT != 0) && (self.interval > 0);
    }

    /// remove timeout from any timing wheel or expired queue (okay if on neither)
    pub fn delete(&mut self) {
        unsafe { timeout_del(self) };
    }

    pub fn run_cb(&self) -> bool {
        let cb = self.callback;
        return cb.call();
    }
}

impl Drop for Timeout {
    fn drop(&mut self) {
        self.delete(); // delete
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimeoutSItFlag {
    PENDING = TIMEOUTS_PENDING as isize,
    EXPIRED = TIMEOUTS_EXPIRED as isize,
    ALL = TIMEOUTS_ALL as isize,
    // CLEAR = TIMEOUTS_CLEAR as isize,
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
            // TIMEOUTS_CLEAR => TimeoutSItFlag::CLEAR, // CLEAR means clear all expired timeout on expire queue.
            // this creates complications in ownership
            _ => TimeoutSItFlag::ALL,
        }
    }
}

pub struct TimeoutSIt {
    raw: NonNull<timeouts_it>,
}

impl TimeoutSIt {
    /// flag has 3 value: PENDING, EXPIRED, ALL
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

/// expired timeout return type
pub enum TOR {
    OneTime(Timeout),           // instance ownership from C side to rust
    Periodic(&'static Timeout), // instance ownership still on C side
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
    /// add Timeout to timing wheel
    /// Timeout type:
    /// - INT: first expired on now + timeout, then expired at now + timeout + timeout + ...
    ///        even if it's expired,TimeoutManger will not auto renew it. must consume it use expired_timeouts.
    /// - ABS: expired on timeout, then expired only once.
    /// - Default: first expired on now + timeout, then expired only once.
    /// Pass in ownership of the Timeout object
    pub fn add(&mut self, to: Timeout, timeout: timeout_t) {
        let to_ptr = Box::into_raw(Box::new(to));
        unsafe { timeouts_add(self.get_raw(), to_ptr, timeout) };
    }
    /// remove Timeout from any timing wheel or expired queue (okay if on neither)
    /// No ownership of the Timeout object is passed in
    pub fn delete(&mut self, to: &mut Timeout) {
        unsafe { timeouts_del(self.get_raw(), to) };
    }

    /// consume expired timeout
    /// return next expired timeout, or NULL if none
    /// TOR::OneTime: Ownership of timeout objects moved from C to rust
    ///               all pending/expired flag has cleared.
    /// TOR::Periodic: Ownership still on C side
    pub(crate) fn expired_timeout<'a>(&'a mut self) -> Option<TOR> {
        let to_ptr: *mut timeout = unsafe { timeouts_get(self.get_raw()) };
        if to_ptr.is_null() {
            return None;
        }
        if unsafe { (*to_ptr).is_periodic() } {
            return Some(TOR::Periodic(unsafe { &*to_ptr }));
        }
        return Some(TOR::OneTime(Timeout::transfer(to_ptr)));
        // return Some(Timeout::transfer(to_ptr));
    }
    /// return next expired timeout iterator
    pub fn expired_timeouts(&mut self) -> ToMIterExpire {
        ToMIterExpire {
            timeout_manager: self,
        }
    }
    /// return next quote of timeout as timeout_sit requested, or NULL if none
    /// No ownership of the Timeout object is passed in from C to rust
    /// No consume any timeout
    //
    pub(crate) fn next_timeout<'a, 'b>(
        &'a mut self,
        timeout_sit: &'b TimeoutSIt,
    ) -> Option<&'b Timeout> {
        let to_ptr: *mut timeout =
            unsafe { timeouts_next(self.get_raw(), timeout_sit.raw.as_ptr()) };
        if to_ptr.is_null() {
            return None;
        }
        return Some(unsafe { &*to_ptr });
    }
    /// return next timeout quote iterator(as requested by timeout_sit)
    pub fn next_timeouts<'a>(&'a mut self, timeout_sit: &'a TimeoutSIt) -> ToMIterNext<'a> {
        ToMIterNext {
            timeout_manager: self,
            timeout_sit,
        }
    }
}

impl Drop for TimeoutManager {
    fn drop(&mut self) {
        self.close();
    }
}

pub struct ToMIterExpire<'a> {
    timeout_manager: &'a mut TimeoutManager,
}

impl<'a> Iterator for ToMIterExpire<'a> {
    type Item = TOR;
    fn next(&mut self) -> Option<Self::Item> {
        self.timeout_manager.expired_timeout()
    }
}

pub struct ToMIterNext<'a> {
    timeout_manager: &'a mut TimeoutManager,
    timeout_sit: &'a TimeoutSIt,
}

impl<'a> Iterator for ToMIterNext<'a> {
    type Item = &'a Timeout;
    fn next(&mut self) -> Option<Self::Item> {
        self.timeout_manager.next_timeout(self.timeout_sit)
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
        let to = Timeout::new(TimeoutType::Default).unwrap(); // relative timeout
        assert!(!to.is_pending());
        assert!(!to.is_expired());

        let to2 = Timeout::new(TimeoutType::INT).unwrap(); // relative timeout
        assert!(!to2.is_pending());
        assert!(!to2.is_expired());

        let mut tos = TimeoutManager::new(TIMEOUT_mHZ).unwrap();
        tos.update_time_int(0); // tos.now = 0
        tos.add(to, 100); // onetime, expired time = tos.now + 100
        tos.add(to2, 100); // periodic

        let tos_it = TimeoutSIt::new(TimeoutSItFlag::PENDING).unwrap();
        let to = tos.next_timeout(&tos_it).unwrap();
        let to2 = tos.next_timeout(&tos_it).unwrap();

        tos.update_time_int(1); // tos.now = 1
        assert!(to.is_pending());
        assert!(!to.is_expired());

        tos.update_time_int(98); // tos.now = 99
        assert!(to.is_pending());
        assert!(!to.is_expired());

        tos.update_time_int(10); // tos.now = 109

        let tos_it = TimeoutSIt::new(TimeoutSItFlag::EXPIRED).unwrap();
        for to in tos.next_timeouts(&tos_it) {
            assert!(!to.is_pending());
            assert!(to.is_expired());
        }

        for to in tos.expired_timeouts() {
            match to {
                TOR::OneTime(temp) => {
                    // all flag has cleared
                    // assert!(to.is_expired());
                }
                TOR::Periodic(temp) => {
                    assert!(temp.is_periodic()); // next expired time = 200
                }
            };
        }

        tos.update_time_int(110); // tos.now = 219

        let mut temp_flag = false;

        for to in tos.expired_timeouts() {
            match to {
                TOR::OneTime(temp) => {
                    // all flag has cleared
                    // assert!(to.is_expired());
                }
                TOR::Periodic(temp) => {
                    assert!(temp.is_periodic());
                    temp_flag = true;
                }
            };
        }

        assert!(temp_flag);
    }

    #[test]
    fn test_timeout_sit_flag_into_i32() {
        let pending = TimeoutSItFlag::PENDING;
        let expired = TimeoutSItFlag::EXPIRED;
        let all = TimeoutSItFlag::ALL;

        assert_eq!(i32::from(pending), TIMEOUTS_PENDING);
        assert_eq!(i32::from(expired), TIMEOUTS_EXPIRED);
        assert_eq!(i32::from(all), TIMEOUTS_ALL);

        assert_eq!(TimeoutSItFlag::new(TIMEOUTS_PENDING), pending);
        assert_eq!(TimeoutSItFlag::new(TIMEOUTS_EXPIRED), expired);
        assert_eq!(TimeoutSItFlag::new(TIMEOUTS_ALL), all);
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
        tos.add(timeout, 100); // expired time = tos.now + 100

        let tos_it = TimeoutSIt::new(TimeoutSItFlag::PENDING).unwrap();
        let to = tos.next_timeout(&tos_it);
        let to = to.unwrap();

        tos.update_time_abs(30);
        assert!(tos.any_pending());
        assert!(!tos.any_expired());
        assert!(tos.get_next_wait_time() < 70);

        assert!(to.is_pending());
        assert!(!to.is_expired());

        tos.update_time_abs(100);
        assert!(!tos.any_pending());
        assert!(tos.any_expired());
        assert_eq!(tos.get_next_wait_time(), 0);

        tos.update_time_abs(150);

        assert!(!to.is_pending());
        assert!(to.is_expired());

        let timeout2 = tos.expired_timeout();
        assert!(timeout2.is_some());
    }

    #[test]
    #[allow(unused_variables)]
    fn test_callback() {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct Session {
            pub session_id: String,
        }
        impl Drop for Session {
            fn drop(&mut self) {
                println!("drop session: {}", self.session_id);
            }
        }

        // // callback
        // extern "C" fn rust_callback(arg: *mut c_void) {
        //     let value = unsafe { *(arg as *mut i32) };
        //     println!("Callback executed with arg: {}", value);
        // }
        // let arg: i32 = 42;
        // let callback = TimeoutCallBack::new(rust_callback, &arg as *const _ as *mut c_void);
        // timeout.set_cb(callback);

        let mut timeout = Timeout::new(TimeoutType::Default).unwrap(); // relative timeout
                                                                       // callback
                                                                       // extern "C" fn rust_callback(arg: *mut c_void) {
                                                                       //     let value: Box<Rc<RefCell<Session>>> =
                                                                       //         unsafe { Box::from_raw(arg as *mut Rc<RefCell<Session>>) };

        //     let value = value.borrow();
        //     println!("Callback executed with arg: {}", value.session_id);
        // }

        // let session = Session {
        //     session_id: "123".to_string(),
        // };
        // let session_ref = Rc::new(RefCell::new(session));
        // let arg = Box::into_raw(Box::new(session_ref.clone()));

        // let callback = TimeoutCallBack::new(rust_callback, arg as *const _ as *mut c_void);
        // timeout.set_cb(callback);
        // session_ref.borrow_mut().session_id = "456".to_string();
        // timeout.run_cb();

        extern "C" fn rust_callback2(arg: *mut c_void) {
            let value = arg as *mut Rc<RefCell<Session>>;
            let value = unsafe { &mut *value };

            println!("rust_callback2 count: {}", Rc::strong_count(value));

            // let value = value.borrow();
            println!("Callback executed with arg: {}", value.borrow().session_id);
            drop(value);
            // println!("rust_callback2 count: {}", Rc::strong_count(value));
        }

        {
            let s2 = Session {
                session_id: "2123".to_string(),
            };
            let s2_ref = Rc::new(RefCell::new(s2));
            // let arg3 = Rc::clone(&s2_ref);
            // let arg2 = &mut s2_ref.clone();
            // let arg2 = Rc::into_raw(s2_ref.clone());

            let arg2 = Box::into_raw(Box::new(s2_ref.clone()));

            // GET RFE COUNT FOROM S2
            println!("s2_ref count: {}", Rc::strong_count(&s2_ref));

            let callback = TimeoutCallBack::new(rust_callback2, arg2 as *const _ as *mut c_void);
            timeout.set_cb(callback);
        }

        // timeout.set_cb(callback);
        timeout.run_cb();

        // s2_ref.borrow_mut().session_id = "2456".to_string();
        // timeout.run_cb();
        println!("aaaa");
        // timeout.run_cb();
        // println!("s2_ref count: {}", Rc::strong_count(&s2_ref));
    }
}
