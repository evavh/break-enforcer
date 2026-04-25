use std::cell::Cell;
use std::sync::Mutex;
use std::thread::JoinHandle;

use std::time::Duration;
use std::time::Instant;

use crate::InstantExt;
use crate::run::NewState;
use crate::run::ResetOrBreak;
use crate::run::state_machine::{Devices, UserTracking};

pub struct TestParams {
    time: Option<Instant>,
    locked: bool,
    moving: bool,
    pub parked: bool,
    pub state: Option<NewState>,
}

impl TestParams {
    const fn new() -> Self {
        Self {
            time: None,
            locked: false,
            moving: false,
            parked: false,
            state: None,
        }
    }
}

thread_local! {
    pub static PARAMS: Cell<Option<&'static Mutex<TestParams>>> = const { Cell::new(None) };
    pub static THREAD: Cell<Option<JoinHandle<()>>> = const { Cell::new(None) };
}

pub fn params() -> std::sync::MutexGuard<'static, TestParams> {
    PARAMS
        .get()
        .expect("test params should be set by `spawn_in_mocked_environment`")
        .lock()
        .unwrap()
}

pub fn now() -> Instant {
    if let Some(now) = params().time {
        now
    } else {
        let now = Instant::now();
        params().time = Some(now);
        now
    }
}

pub struct FakeDevices;

impl Devices for FakeDevices {
    type LockGuard = LockGuard;
    fn lock(&self) -> color_eyre::Result<Vec<LockGuard>> {
        params().locked = true;
        Ok(vec![LockGuard])
    }
}

pub struct LockGuard;
impl Drop for LockGuard {
    fn drop(&mut self) {
        params().locked = false;
    }
}

pub struct FakeUserTracking;
impl UserTracking for FakeUserTracking {
    fn reset_or_time_for_break(
        &mut self,
        closest_break: &crate::run::Break,
    ) -> color_eyre::Result<ResetOrBreak> {
        params().moving = false;
        let mut idle_since: Option<Instant> = None;

        loop {
            park();
            if closest_break.next_at.in_the_past() {
                return Ok(ResetOrBreak::Break);
            }
            if let Some(idle_since) = idle_since
                && idle_since.elapsed() > closest_break.duration
                && !params().moving
            {
                return Ok(ResetOrBreak::Reset {
                    idle_time: idle_since.elapsed(),
                });
            }

            idle_since = if params().moving {
                Some(idle_since.unwrap_or_else(|| now()))
            } else {
                None
            }
        }
    }

    fn idle_until_input(&self) -> color_eyre::Result<Duration> {
        params().moving = false;
        let start = now();
        loop {
            park();
            if dbg!(params().moving) {
                break;
            }
        }

        Ok(start.elapsed())
    }
}

pub fn park() {
    // hold mutex to prevent race
    params().parked = true;
    std::thread::park();
}

pub fn sleep(dur: Duration) {
    let mut p = params();
    p.time = Some(p.time.unwrap_or(Instant::now()) + dur);
}

pub fn start_moving() {
    params().moving = true
}

pub fn stop_moving() {
    params().moving = false
}

pub fn spawn_in_mocked_environment(f: impl FnOnce() + Send + 'static) {
    let params = Box::leak(Box::new(Mutex::new(TestParams::new())));
    THREAD.set(Some(std::thread::spawn(|| {
        PARAMS.set(Some(params));
        f();
    })));
    PARAMS.set(Some(params));
}

pub fn unpark() {
    let handle = THREAD
        .take()
        .expect("unpark should only be called after the test has been started");

    // hold mutex to prevent race
    let mut params = params();
    handle.thread().unpark();
    THREAD.set(Some(handle));
    params.parked = false;
}

pub fn advance_time_by(dur: Duration) {
    params().time = Some(now() + dur);
    unpark();
}
