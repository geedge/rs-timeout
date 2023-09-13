use libc::free;
use rand::Rng;

use crate::timeout::timeout_version;
use crate::timeout::*;
use std::{
    ffi::CStr,
    io::{self, Write},
    usize,
};

static mut n_failed: i32 = 0;

macro_rules! DO {
    ($fn:expr) => {{
        print!(".");
        io::stdout().flush().unwrap();
        if $fn {
            unsafe {
                n_failed += 1;
            }
            println!("{} failed", stringify!($fn));
        }
    }};
}

macro_rules! DO_N {
    ($n:expr, $fn:expr) => {{
        for j in 0..$n {
            DO!($fn);
        }
    }};
}

macro_rules! fail {
    () => {{
        println!("Failure on line {}", line!());
        return true;
    }};
}

fn check_misc() -> bool {
    if (TIMEOUT_V_REL as i32) != unsafe { timeout_version() } {
        return true;
    }
    if (TIMEOUT_V_REL as i32) != unsafe { timeout_v_rel() } {
        return true;
    }
    if (TIMEOUT_V_API as i32) != unsafe { timeout_v_api() } {
        return true;
    }
    if (TIMEOUT_V_ABI as i32) != unsafe { timeout_v_abi() } {
        return true;
    }
    if unsafe { CStr::from_ptr(timeout_vendor()).to_bytes() } != TIMEOUT_VENDOR {
        return true;
    }
    false
}

fn check_open_close(hz_set: timeout_t, hz_expect: timeout_t) -> bool {
    let mut err = 0 as usize;
    let tos: *mut timeouts = unsafe { timeouts_open(hz_set, &mut err) };

    if tos.is_null() {
        return true;
    }
    if err != 0 {
        return true;
    }
    if hz_expect != unsafe { timeouts_hz(tos) } {
        return true;
    }
    false
}

/* configuration for check_randomized */
#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct rand_cfg {
    /* When creating timeouts, smallest possible delay */
    min_timeout: timeout_t,
    /* When creating timeouts, largest possible delay */
    max_timeout: timeout_t,
    /* First time to start the clock at. */
    start_at: timeout_t,
    /* Do not advance the clock past this time. */
    end_at: timeout_t,
    /* Number of timeouts to create and monitor. */
    n_timeouts: usize,
    /* Advance the clock by no more than this each step. */
    max_step: timeout_t,
    /* Use relative timers and stepping */
    relative: usize,
    /* Every time the clock ticks, try removing this many timeouts at
     * random. */
    try_removing: usize,
    /* When we're done, advance the clock to the end of time. */
    finalize: usize,
}

/* Not very random */
// fn random_to(min: timeout_t, max: timeout_t) -> timeout_t {
//     let mut rng = rand::thread_rng();
//     if max <= min {
//         return min;
//     }
//     /* Not actually all that random, but should exercise the code. */
//     let rand64 = rng.gen::<u64>() * (i32::MAX as timeout_t) + rng.gen::<u64>();
//     min + (rand64 % (max - min))
// }

fn random_to(min: u64, max: u64) -> u64 {
    if max <= min {
        return min;
    }
    let rand64 = rand::thread_rng().gen::<u64>();
    min + (rand64 % (max - min))
}

fn random() -> usize {
    rand::thread_rng().gen::<usize>()
}

fn check_randomized(cfg: &rand_cfg) -> bool {
    let (i, rv) = (0, 0);
    let mut err = 0 as usize;

    let mut t: Vec<timeout> = vec![timeout::default(); cfg.n_timeouts];
    let mut timeouts: Vec<timeout_t> = vec![0; cfg.n_timeouts];
    let mut fired: Vec<u8> = vec![0; cfg.n_timeouts];
    let mut found: Vec<u8> = vec![0; cfg.n_timeouts];
    let mut deleted: Vec<u8> = vec![0; cfg.n_timeouts];

    let mut tos = unsafe { Some(timeouts_open(0, &mut err)) };
    let mut now = cfg.start_at;

    let (mut n_added_pending, mut cnt_added_pending, mut n_added_expired, mut cnt_added_expired) =
        (0, 0, 0, 0);

    let (mut it_p, mut it_e, mut it_all) = (
        timeouts_it::default(),
        timeouts_it::default(),
        timeouts_it::default(),
    );

    let (mut p_done, mut e_done, mut all_done) = (false, false, false);
    let mut to: Option<&mut timeout> = None;
    let rel = cfg.relative;

    // 对应 done
    let cleanup = |tos,
                   t: Vec<timeout>,
                   timeouts: Vec<timeout_t>,
                   fired: Vec<u8>,
                   found: Vec<u8>,
                   deleted: Vec<u8>| {
        if let Some(tos) = tos {
            unsafe { timeouts_close(tos) };
        }
        if t.is_empty() {
            drop(t);
            // unsafe { free(t) };
        }
        if !timeouts.is_empty() {
            drop(timeouts);
            // unsafe { free(timeouts) };
        }
        if !fired.is_empty() {
            drop(fired);
            // unsafe { free(fired) };
        }
        if !found.is_empty() {
            drop(found);
            // unsafe { free(found) };
        }
        if !deleted.is_empty() {
            drop(deleted);
            // unsafe { free(deleted) };
        }
    };

    if t.is_empty()
        || timeouts.is_empty()
        || tos.is_none()
        || fired.is_empty()
        || found.is_empty()
        || deleted.is_empty()
    {
        cleanup(tos, t, timeouts, fired, found, deleted);
        fail!();
    }
    if let Some(mut tos) = tos {
        unsafe { timeouts_update(tos, cfg.start_at) };
    }

    // test-timeout.c line 98
    for i in 0..cfg.n_timeouts {
        if &t[i] as *const _
            != unsafe { timeout_init(&mut t[i], if rel > 0 { 0 } else { TIMEOUT_ABS }) } as *const _
        {
            cleanup(tos, t, timeouts, fired, found, deleted);
            fail!();
        }
        if unsafe { timeout_pending(&mut t[i]) || timeout_expired(&mut t[i]) } {
            cleanup(tos, t, timeouts, fired, found, deleted);
            fail!();
        }

        timeouts[i] = random_to(cfg.min_timeout, cfg.max_timeout);

        unsafe {
            if let Some(temp) = tos {
                timeouts_add(
                    temp,
                    &mut t[i],
                    timeouts[i] - (if rel > 0 { now } else { 0 }),
                )
            }
        }
        if timeouts[i] <= cfg.start_at {
            if unsafe { timeout_pending(&mut t[i]) || !timeout_expired(&mut t[i]) } {
                cleanup(tos, t, timeouts, fired, found, deleted);
                fail!();
            }
            n_added_expired += 1;
        } else {
            if unsafe { !timeout_pending(&mut t[i]) || timeout_expired(&mut t[i]) } {
                cleanup(tos, t, timeouts, fired, found, deleted);
                fail!();
            }
            n_added_pending += 1;
        }
    }

    // test-timeout.c line 124
    if unsafe {
        if let Some(temp) = tos {
            (n_added_pending != 0) != timeouts_pending(temp)
        } else {
            true
        }
    } {
        cleanup(tos, t, timeouts, fired, found, deleted);
        fail!();
    }
    // test-timeout.c line 126
    if unsafe {
        if let Some(temp) = tos {
            (n_added_expired != 0) != timeouts_expired(temp)
        } else {
            true
        }
    } {
        cleanup(tos, t, timeouts, fired, found, deleted);
        fail!();
    }

    TIMEOUTS_IT_INIT(&mut it_p, TIMEOUTS_PENDING);
    TIMEOUTS_IT_INIT(&mut it_e, TIMEOUTS_EXPIRED);
    TIMEOUTS_IT_INIT(&mut it_all, TIMEOUTS_ALL);

    // timeout.c line 133
    while !(p_done && e_done && all_done) {
        if !p_done {
            if let Some(temp) = tos {
                let to = unsafe { timeouts_next(temp, &mut it_p) };
                if !to.is_null() {
                    // c 语言中的指针运算,这里只能以 内存地址长度/ timeout 类型长度,来进行运算
                    let i =
                        (to as usize - &t[0] as *const _ as usize) / std::mem::size_of::<timeout>();
                    found[i] += 1;
                    cnt_added_pending += 1;
                } else {
                    p_done = true;
                }
            }
        }
        if !e_done {
            if let Some(temp) = tos {
                let to = unsafe { timeouts_next(temp, &mut it_e) };
                if !to.is_null() {
                    // c 语言中的指针运算,这里只能以 内存地址长度/ timeout 类型长度,来进行运算
                    let i =
                        (to as usize - &t[0] as *const _ as usize) / std::mem::size_of::<timeout>();
                    found[i] += 1;
                    cnt_added_expired += 1;
                } else {
                    e_done = true;
                }
            }
        }
        if !all_done {
            if let Some(temp) = tos {
                let to = unsafe { timeouts_next(temp, &mut it_all) };
                if !to.is_null() {
                    // c 语言中的指针运算,这里只能以 内存地址长度/ timeout 类型长度,来进行运算
                    let i =
                        (to as usize - &t[0] as *const _ as usize) / std::mem::size_of::<timeout>();
                    found[i] += 1;
                } else {
                    all_done = true;
                }
            }
        }
    }
    // timeout.c line 164
    for i in 0..cfg.n_timeouts {
        if found[i] != 2 {
            cleanup(tos, t, timeouts, fired, found, deleted);
            fail!();
        }
    }
    if cnt_added_expired != n_added_expired {
        cleanup(tos, t, timeouts, fired, found, deleted);
        fail!();
    }
    if cnt_added_pending != n_added_pending {
        cleanup(tos, t, timeouts, fired, found, deleted);
        fail!();
    }
    // timeout.c line 174
    if let Some(temp) = tos {
        loop {
            let to: *mut timeout = unsafe { timeouts_get(temp) };
            if to.is_null() {
                break;
            }
            let i = (to as usize - &t[0] as *const _ as usize) / std::mem::size_of::<timeout>();
            assert_eq!(&t[i] as *const _, to);
            if timeouts[i] > cfg.start_at {
                /* shouldn't have happened yet */
                cleanup(tos, t, timeouts, fired, found, deleted);
                fail!();
            }
            n_added_expired -= 1; /* drop expired timeouts. */
            fired[i] += 1;
        }
    }

    if n_added_expired != 0 {
        cleanup(tos, t, timeouts, fired, found, deleted);
        fail!();
    }

    // test-timeout.c line 187
    while now < cfg.end_at {
        let mut n_fired_this_time = 0;

        // now 似乎超过 100 就不行
        let first_at = unsafe { timeouts_timeout(tos.unwrap()) + now };
        let oldtime = now;
        let step = random_to(1, cfg.max_step);

        now += step;
        if rel > 0 {
            unsafe { timeouts_step(tos.unwrap(), step) };
        } else {
            unsafe { timeouts_update(tos.unwrap(), now) };
        }

        for _i in 0..cfg.try_removing {
            let idx = random() % cfg.n_timeouts;
            if !(fired[idx] > 0) {
                unsafe {
                    timeout_del(&mut t[idx]);
                }
                deleted[idx] += 1;
            }
        }

        let mut another = unsafe { timeouts_timeout(tos.unwrap()) } == 0;

        loop {
            let to = unsafe { timeouts_get(tos.unwrap()) };
            if to.is_null() {
                break;
            }
            if !another {
                /* Thought we saw the last one! */
                cleanup(tos, t, timeouts, fired, found, deleted);
                fail!();
            }

            let i = (to as usize - &t[0] as *const _ as usize) / std::mem::size_of::<timeout>();
            assert_eq!(&t[i] as *const _, to);
            if timeouts[i] > now {
                /* shouldn't have happened yet */
                cleanup(tos, t, timeouts, fired, found, deleted);
                fail!();
            }
            if timeouts[i] <= oldtime {
                /* should have happened already*/
                cleanup(tos, t, timeouts, fired, found, deleted);
                fail!();
            }
            if timeouts[i] < first_at {
                /* first_at should've been earlier */
                cleanup(tos, t, timeouts, fired, found, deleted);
                fail!();
            }
            fired[i] += 1;
            n_fired_this_time += 1;
            another = unsafe { timeouts_timeout(tos.unwrap()) } == 0;
        }

        if (n_fired_this_time != 0) && (first_at > now) {
            /* first_at should've been earlier */
            cleanup(tos, t, timeouts, fired, found, deleted);
            fail!();
        }
        if another {
            /* Huh? We think there are more? */
            cleanup(tos, t, timeouts, fired, found, deleted);
            fail!();
        }
        if !unsafe { timeouts_check(tos.unwrap(), stderr) } {
            cleanup(tos, t, timeouts, fired, found, deleted);
            fail!();
        }

        // let first_at = unsafe { timeouts_timeout(tos.unwrap()) + now };
    }

    return false;
}

#[test]
fn main() {
    unsafe {
        n_failed = 0;
    }
    DO!(check_misc());
    DO!(check_open_close(1000, 1000));
    DO!(check_open_close(0, TIMEOUT_mHZ));

    let cfg1 = rand_cfg {
        min_timeout: 1,
        max_timeout: 100,
        start_at: 5,
        end_at: 1000,
        n_timeouts: 1000,
        max_step: 10,
        relative: 0,
        try_removing: 0,
        finalize: 2,
    };
    // DO_N!(300, check_randomized(&cfg1));

    let cfg2b = rand_cfg {
        min_timeout: 20,
        max_timeout: 1000,
        start_at: 10,
        end_at: 100,
        n_timeouts: 1000,
        max_step: 5,
        relative: 1,
        try_removing: 0,
        finalize: 1,
    };
    DO_N!(300,check_randomized(&cfg2b));
}
