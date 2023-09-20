use libc::free;
use rand::Rng;

use crate::timeout_bind::timeout_version;
use crate::timeout_bind::*;
use std::{
    ffi::CStr,
    io::{self, Write},
    usize,
};

static mut n_failed: i32 = 0;
const THE_END_OF_TIME: u64 = std::u64::MAX;

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
    // 实际上是 bool 值
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

    let mut tos = unsafe { Some(timeouts_open(0, &mut err)) }; // manager 的角色
    let mut now = cfg.start_at;

    let (mut n_added_pending, mut cnt_added_pending, mut n_added_expired, mut cnt_added_expired) =
        (0, 0, 0, 0);

    let (mut it_p, mut it_e, mut it_all) = (
        timeouts_it::default(),
        timeouts_it::default(),
        timeouts_it::default(),
    );

    let (mut p_done, mut e_done, mut all_done) = (false, false, false);
    // let mut to: Option<&mut timeout> = None;
    let mut to: *mut timeout = std::ptr::null_mut();
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
        unsafe { timeouts_update(tos, cfg.start_at) }; // manger 写入开始时间
    }

    // test-timeout.c line 98
    for i in 0..cfg.n_timeouts {
        // 初始化 timeout
        if &t[i] as *const _
            != unsafe { timeout_init(&mut t[i], if rel > 0 { 0 } else { TIMEOUT_ABS }) } as *const _
        {
            cleanup(tos, t, timeouts, fired, found, deleted);
            fail!();
        }
        // 检查 timeout 的状态
        if unsafe { timeout_pending(&mut t[i]) || timeout_expired(&mut t[i]) } {
            cleanup(tos, t, timeouts, fired, found, deleted);
            fail!();
        }

        timeouts[i] = random_to(cfg.min_timeout, cfg.max_timeout); // 随机取超时时间 [min_timeout, max_timeout)]

        unsafe {
            if let Some(temp) = tos {
                timeouts_add(
                    temp,                                          // manger
                    &mut t[i],                                     // timeout
                    timeouts[i] - (if rel > 0 { now } else { 0 }), // 超时时间
                )
            }
        }
        // 超时时间 小于 开始时间?
        if timeouts[i] <= cfg.start_at {
            // 如果还在等待 且 没有过期
            if unsafe { timeout_pending(&mut t[i]) || !timeout_expired(&mut t[i]) } {
                cleanup(tos, t, timeouts, fired, found, deleted);
                fail!();
            }
            n_added_expired += 1; // 计数
        } else {
            // 如果 没有等待 且 已经过期了
            if unsafe { !timeout_pending(&mut t[i]) || timeout_expired(&mut t[i]) } {
                cleanup(tos, t, timeouts, fired, found, deleted);
                fail!();
            }
            n_added_pending += 1; // 计数
        }
    }

    // test-timeout.c line 124
    // 检查 等待事件数量 与 计数 是否相等
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
    // 检查 过期事件数量 与 计数 是否相等
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

    TIMEOUTS_IT_INIT(&mut it_p, TIMEOUTS_PENDING); // 等待
    TIMEOUTS_IT_INIT(&mut it_e, TIMEOUTS_EXPIRED); // 过期
    TIMEOUTS_IT_INIT(&mut it_all, TIMEOUTS_ALL); // 全部

    // timeout.c line 133
    while !(p_done && e_done && all_done) {
        if !p_done {
            if let Some(temp) = tos {
                let to = unsafe { timeouts_next(temp, &mut it_p) }; // 等待队列的下一个
                if !to.is_null() {
                    // c 语言中的指针运算,这里只能以 内存地址长度/ timeout 类型长度,来进行运算
                    // 找到 to 在 t[] 的位置
                    let i =
                        (to as usize - &t[0] as *const _ as usize) / std::mem::size_of::<timeout>();
                    found[i] += 1; // 事件的状态
                    cnt_added_pending += 1;
                } else {
                    // to 为空,没有等待的 timeout 了
                    p_done = true;
                }
            }
        }
        if !e_done {
            if let Some(temp) = tos {
                let to = unsafe { timeouts_next(temp, &mut it_e) }; // 过期队列的下一个
                if !to.is_null() {
                    // c 语言中的指针运算,这里只能以 内存地址长度/ timeout 类型长度,来进行运算
                    let i =
                        (to as usize - &t[0] as *const _ as usize) / std::mem::size_of::<timeout>();
                    found[i] += 1;
                    cnt_added_expired += 1;
                } else {
                    // to 为空,没有过期的 timeout 了
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
                    // to 为空,没有 timeout 了
                    all_done = true;
                }
            }
        }
    }
    // timeout.c line 164
    for i in 0..cfg.n_timeouts {
        // 事件经历 等待 or 超时 + all 两次
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
            let to: *mut timeout = unsafe { timeouts_get(temp) }; // 任何已经过期的事件, 直到没有超时事件
            if to.is_null() {
                break;
            }
            let i = (to as usize - &t[0] as *const _ as usize) / std::mem::size_of::<timeout>(); // 找到 to 在 t[] 的位置
            assert_eq!(&t[i] as *const _, to);
            if timeouts[i] > cfg.start_at {
                //已过期的 事件绝对
                /* shouldn't have happened yet */
                cleanup(tos, t, timeouts, fired, found, deleted);
                fail!();
            }
            n_added_expired -= 1; /* drop expired timeouts. */
            fired[i] += 1;
        }
    }
    // 所有 过期事件 已经经过遍历了
    if n_added_expired != 0 {
        cleanup(tos, t, timeouts, fired, found, deleted);
        fail!();
    }

    // test-timeout.c line 187
    // 时间流逝到 end_at
    while now < cfg.end_at {
        let mut n_fired_this_time = 0;

        // test-timeout.c line 189
        // timeouts_timeout 取到一定程度会返回 u64 最大值,
        // c 语言中 max_u64 + now == now -1 ; 但 rust 会直接 panic;
        let temp = unsafe { timeouts_timeout(tos.unwrap()) }; // 下一次更新所需要的间隔 ??
        let first_at = match temp.checked_add(now) {
            Some(v) => v,
            None => now - 1, // 溢出则按照 C 语言执行结果处理 -1;
        }; // 过期时间 第一个事件的 时间

        let oldtime = now; // 记录上一次的时间
        let step = random_to(1, cfg.max_step); // 随机取步长 [1, max_step)
        now += step; // 时间流逝

        if rel != 0 {
            unsafe { timeouts_step(tos.unwrap(), step) }; // 相对时间更新 计时轮
        } else {
            unsafe { timeouts_update(tos.unwrap(), now) }; // 绝对时间更新 计时轮
        }
        // 每次时钟 滴答时,尝试 随机删除 try_removing 个 timeout
        for _i in 0..cfg.try_removing {
            let idx = random() % cfg.n_timeouts; // 随机取个 timeout
                                                 // 超时已经被遍历过了
            if !(fired[idx] == 0) {
                unsafe {
                    timeout_del(&mut t[idx]);
                }
                deleted[idx] += 1;
            }
        }

        let mut another = unsafe { timeouts_timeout(tos.unwrap()) } == 0; // 下一次更新所需要的间隔 是否 == 0,

        loop {
            let to = unsafe { timeouts_get(tos.unwrap()) }; // 任何已经过期的事件, 直到没有超时事件
            if to.is_null() {
                break;
            }
            if !another {
                /* Thought we saw the last one! */
                cleanup(tos, t, timeouts, fired, found, deleted);
                fail!();
            }

            let i = (to as usize - &t[0] as *const _ as usize) / std::mem::size_of::<timeout>(); // 找到 to 在 t[] 的位置
            assert_eq!(&t[i] as *const _, to);
            // 已经过期时间 超时时间不会比现在还小
            if timeouts[i] > now {
                /* shouldn't have happened yet */
                cleanup(tos, t, timeouts, fired, found, deleted);
                fail!();
            }
            // 已经过期时间 超时时间不会比上一次的时间还小 | 上次的过期事件绝对在上次已经处理完毕了.
            if timeouts[i] <= oldtime {
                /* should have happened already*/
                cleanup(tos, t, timeouts, fired, found, deleted);
                fail!();
            }
            // 已经过期时间 超时时间不会比 这次更新需要的最小 时间间隔还小
            if timeouts[i] < first_at {
                /* first_at should've been earlier */
                cleanup(tos, t, timeouts, fired, found, deleted);
                fail!();
            }
            fired[i] += 1; // 过期数组 计数
            n_fired_this_time += 1; //
            another = (unsafe { timeouts_timeout(tos.unwrap()) } == 0);
        }
        // 这轮处理过 超时事件 且 第一个超时事件的时间点 > now
        if (n_fired_this_time != 0) && (first_at > now) {
            /* first_at should've been earlier */
            // 这里的处理逻辑是,如果有超时事件,那么第一个超时事件的时间点应该是 <= now
            cleanup(tos, t, timeouts, fired, found, deleted);
            fail!();
        }
        // 这轮处理过 超时事件 且 下一次更新所需要的间隔 == 0, 不可能出现.
        if another {
            /* Huh? We think there are more? */
            cleanup(tos, t, timeouts, fired, found, deleted);
            fail!();
        }
        // 有效??
        if unsafe { !timeouts_check(tos.unwrap(), stderr) } {
            cleanup(tos, t, timeouts, fired, found, deleted);
            fail!();
        }
    }

    // test-timout.c line 233
    for i in 0..cfg.n_timeouts {
        // 过期 了两次
        if fired[i] > 1 {
            /* Nothing fired twice. */
            cleanup(tos, t, timeouts, fired, found, deleted);
            fail!();
        }
        // 超时时间 小于 开始时间?
        if timeouts[i] <= now {
            // 已经过期了, 却 还没有过期 且 没有删除 ?
            if (fired[i] == 0) && (deleted[i] == 0) {
                cleanup(tos, t, timeouts, fired, found, deleted);
                fail!();
            }
        } else {
            // 没有过期, 但被标记为 已经过期了
            if fired[i] != 0 {
                cleanup(tos, t, timeouts, fired, found, deleted);
                fail!();
            }
        }
        // 过期了 且 删除了
        if (fired[i] != 0) && (deleted[i] != 0) {
            cleanup(tos, t, timeouts, fired, found, deleted);
            fail!();
        }
        // 完成后 将时钟 更新到 timeout 的最大值
        if cfg.finalize > 1 {
            if !(fired[i] != 0) {
                unsafe {
                    timeout_del(&mut t[i]);
                }
            }
        }
    }

    // test-timeout.c line 251
    /* Now nothing more should fire between now and the end of time. */
    // 从 end_t 到 max_t 不应该再有事件了
    if cfg.finalize > 0 {
        // 时间移动到 timeout 的尽头
        unsafe { timeouts_update(tos.unwrap(), THE_END_OF_TIME) }
        //
        if cfg.finalize > 1 {
            // 任何已经过期的事件
            let tmpe = unsafe { timeouts_get(tos.unwrap()) };
            // 如果还有 事件
            if !tmpe.is_null() {
                cleanup(tos, t, timeouts, fired, found, deleted);
                fail!();
            }
            let mut _it = TIMEOUTS_IT_INITIALIZER(TIMEOUTS_ALL); // 所有队列的事件
            loop {
                to = unsafe { timeouts_next(tos.unwrap(), &mut _it) };
                if to.is_null() {
                    break;
                }
                // 如何还有事件
                cleanup(tos, t, timeouts, fired, found, deleted);
                fail!();
            }
        }
    }

    cleanup(tos, t, timeouts, fired, found, deleted);
    return false;
}

struct intervals_cfg<'a> {
    timeouts: &'a [timeout_t],
    n_timeouts: usize,
    start_at: timeout_t,
    end_at: timeout_t,
    skip: timeout_t,
}

fn check_intervals(cfg: &intervals_cfg) -> bool {
    let (mut i, rv) = (0, 0);
    let mut err = 0 as usize;

    let mut to: *mut timeout = std::ptr::null_mut();
    let mut t: Vec<timeout> = vec![timeout::default(); cfg.n_timeouts as usize];
    let mut fired: Vec<u32> = vec![0; cfg.n_timeouts];
    let mut tos = unsafe { Some(timeouts_open(0, &mut err)) }; // manger

    let mut now = cfg.start_at;

    // 对应 done
    let cleanup = |t: Vec<timeout>, tos: Option<*mut timeouts>, fired: Vec<u32>| {
        if t.is_empty() {
            drop(t);
            // unsafe { free(t) };
        }
        if let Some(tos) = tos {
            unsafe { timeouts_close(tos) };
        }
        if !fired.is_empty() {
            drop(fired);
            // unsafe { free(fired) };
        }
    };

    if t.is_empty() || tos.is_none() || fired.is_empty() {
        cleanup(t, tos, fired);
        fail!();
    }

    unsafe { timeouts_update(tos.unwrap(), now) }; // 时钟行进到 now
    // test-timeout.c line 300
    for i in 0..cfg.n_timeouts {
        // 初始化 按照绝对时间 计算超时
        if &t[i] as *const _ != unsafe { timeout_init(&mut t[i], TIMEOUT_INT) } as *const _ {
            cleanup(t, tos, fired);
            fail!();
        }
        // timeout 等待(在时间轮上) 或 过期
        if unsafe { timeout_pending(&mut t[i]) || timeout_expired(&mut t[i]) } {
            cleanup(t, tos, fired);
            fail!();
        }
        // 添加 timeout 到 时间轮上
        unsafe { timeouts_add(tos.unwrap(), &mut t[i], cfg.timeouts[i]) }
        // 没有在时间轮上 且 已过期
        if unsafe { (!timeout_pending(&mut t[i])) || timeout_expired(&mut t[i]) } {
            cleanup(t, tos, fired);
            fail!();
        }
    }
    // test-timeout.c line 315
    while now < cfg.end_at {
        let mut delay = unsafe { timeouts_timeout(tos.unwrap()) }; // 触发下一个 超时事件 最小时间间隔
        // 有 跳过间歇 且 delay < 间歇
        if (cfg.skip != 0) && (delay < cfg.skip) {
            delay = cfg.skip; // delay = 间歇
        }
        unsafe { timeouts_step(tos.unwrap(), delay) } // 更新时钟 (相对方式)
        now += delay; // 时间流逝

        loop {
            let to = unsafe { timeouts_get(tos.unwrap()) }; // 
            if to.is_null() {
                break;
            }
            i = (to as usize - &t[0] as *const _ as usize) / std::mem::size_of::<timeout>();
            assert_eq!(&t[i] as *const _, to);

            fired[i] += 1;

            if 0 != (unsafe { (*to).expires - cfg.start_at } % cfg.timeouts[i]) {
                cleanup(t, tos, fired);
                fail!();
            }
            if unsafe { (*to).expires <= now } {
                cleanup(t, tos, fired);
                fail!();
            }
            if unsafe { (*to).expires > now + cfg.timeouts[i] } {
                cleanup(t, tos, fired);
                fail!();
            }
        }
        if unsafe { !timeouts_check(tos.unwrap(), stderr) } {
            cleanup(t, tos, fired);
            fail!();
        }
    }

    let duration = now - cfg.start_at;
    for i in 0..cfg.n_timeouts {
        if cfg.skip != 0 {
            if fired[i] as u64 > (duration / cfg.timeouts[i]) {
                cleanup(t, tos, fired);
                fail!();
            }
        } else {
            if fired[i] as u64 != (duration / cfg.timeouts[i]) {
                println!("{} != {}", fired[i], duration / cfg.timeouts[i]);
                cleanup(t, tos, fired);
                fail!();
            }
        }
        if unsafe { !timeout_pending(&mut t[i]) } {
            cleanup(t, tos, fired);
            fail!();
        }
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
    DO_N!(300, check_randomized(&cfg1));

    let cfg2 = rand_cfg {
        min_timeout: 20,
        max_timeout: 1000,
        start_at: 5,
        end_at: 100,
        n_timeouts: 1000,
        max_step: 5,
        relative: 1,
        try_removing: 0,
        finalize: 2,
    };
    DO_N!(300, check_randomized(&cfg2));

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
    DO_N!(300, check_randomized(&cfg2b));

    let cfg2c = rand_cfg {
        min_timeout: 20,
        max_timeout: 1000,
        start_at: 10,
        end_at: 100,
        n_timeouts: 1000,
        max_step: 5,
        relative: 1,
        try_removing: 0,
        finalize: 0,
    };
    DO_N!(300, check_randomized(&cfg2c));

    let cfg3 = rand_cfg {
        min_timeout: 2000,
        max_timeout: 1 << 50,
        start_at: 100,
        end_at: 1 << 49,
        n_timeouts: 1000,
        max_step: 1 << 31,
        relative: 0,
        try_removing: 0,
        finalize: 2,
    };
    DO_N!(10, check_randomized(&cfg3));

    let cfg3b = rand_cfg {
        min_timeout: 1 << 50,
        max_timeout: 1 << 52,
        start_at: 100,
        end_at: 1 << 53,
        n_timeouts: 1000,
        max_step: 1 << 48,
        relative: 0,
        try_removing: 0,
        finalize: 2,
    };
    DO_N!(10, check_randomized(&cfg3b));

    let cfg4 = rand_cfg {
        min_timeout: 2000,
        max_timeout: 1 << 30,
        start_at: 100,
        end_at: 1 << 26,
        n_timeouts: 10000,
        max_step: 1 << 16,
        relative: 0,
        try_removing: 0,
        finalize: 2,
    };
    DO_N!(10, check_randomized(&cfg4));

    const primes: [u64; 25] = [
        2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89,
        97,
    ];
    const factors_of_1337: [u64; 4] = [1, 7, 191, 1337];
    const multiples_of_five: [u64; 10] = [5, 10, 15, 20, 25, 30, 35, 40, 45, 50];

    let icfg1 = intervals_cfg {
        timeouts: &primes,
        n_timeouts: primes.len(),
        start_at: 50,
        end_at: 5322,
        skip: 0,
    };
    DO!(check_intervals(&icfg1));

    let icfg2 = intervals_cfg {
        timeouts: &factors_of_1337,
        n_timeouts: factors_of_1337.len(),
        start_at: 50,
        end_at: 50000,
        skip: 0,
    };
    DO!(check_intervals(&icfg2));

    let icfg3 = intervals_cfg {
        timeouts: &multiples_of_five,
        n_timeouts: multiples_of_five.len(),
        start_at: 49,
        end_at: 5333,
        skip: 0,
    };
    DO!(check_intervals(&icfg3));

    let icfg4 = intervals_cfg {
        timeouts: &primes,
        n_timeouts: primes.len(),
        start_at: 50,
        end_at: 5322,
        skip: 16,
    };
    DO!(check_intervals(&icfg4));

    if unsafe { n_failed } != 0 {
        println!("{} tests failed", unsafe { n_failed });
    } else {
        println!("OK");
    }
}
