/// Externally controllable mocked environment
///
/// This fakes time, movement detection and locking. To fake time there are two
/// "times". The simulation time and the targe_time. The simulation will run
/// until the sim time matches the target_time after which the thread will park.
///
/// The target time is externally controlled through the shared TestParams
/// struct. To increase it simply call `advance_time_by(Duration)` from the
/// outside.
use std::cell::Cell;
use std::ops::Add;
use std::ops::AddAssign;
use std::panic;
use std::sync::Mutex;
use std::thread;
use std::thread::JoinHandle;

use std::thread::park;
use std::time::Duration;
use std::time::Instant;

use crate::InstantExt;
use crate::run::IdleOrTimeout;
use crate::run::NewState;
use crate::run::ResetOrBreak;
use crate::run::ScheduledBreak;
use crate::run::state_machine::Time;
use crate::run::state_machine::{Devices, UserTracking};

pub struct TestParams {
    instant_to_run_till: SimulatedInstant,
    locked: bool,
    moving: bool,
    pub parked: bool,
    pub state: Option<NewState<SimulatedInstant>>,
}

impl TestParams {
    const fn new() -> Self {
        Self {
            instant_to_run_till: SimulatedInstant(Duration::ZERO),
            locked: false,
            moving: false,
            parked: false,
            state: None,
        }
    }

    fn time_still_to_run(&self) -> Duration {
        self.instant_to_run_till
            .saturating_duration_since(SimulatedInstant::now())
    }
}

thread_local! {
    pub static PARAMS: Cell<Option<&'static Mutex<TestParams>>> = const { Cell::new(None) };
    pub static THREAD: Cell<Option<JoinHandle<()>>> = const { Cell::new(None) };
    pub static TIME: Cell<SimulatedInstant> = const { Cell::new(SimulatedInstant(Duration::ZERO)) };
}

#[derive(Debug)]
pub struct Timeout;

pub trait MutexExt {
    type Item;
    fn try_lock_within<'a>(
        &'a self,
        dur: Duration,
    ) -> Result<std::sync::MutexGuard<'a, Self::Item>, Timeout>;
}

impl<T> MutexExt for std::sync::Mutex<T> {
    type Item = T;
    fn try_lock_within<'a>(
        &'a self,
        dur: Duration,
    ) -> Result<std::sync::MutexGuard<'a, Self::Item>, Timeout> {
        let start = Instant::now();
        while start.elapsed() < dur {
            if let Ok(t) = self.try_lock() {
                return Ok(t);
            }
        }

        return Err(Timeout);
    }
}

#[track_caller]
pub fn params() -> std::sync::MutexGuard<'static, TestParams> {
    use std::panic::Location;
    static LOCKED_FROM: Mutex<Option<&'static Location<'static>>> =
        Mutex::new(None);

    if let Ok(params) = PARAMS
        .get()
        .expect("test params should be set by `spawn_in_mocked_environment`")
        .try_lock_within(Duration::from_millis(1))
    {
        *LOCKED_FROM.lock().unwrap() = Some(panic::Location::caller());
        params
    } else {
        let location = *LOCKED_FROM.lock().unwrap().unwrap();
        panic!("Deadlocked, params last locked from: {location}",)
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

pub struct SimulatedUserTracking;
impl UserTracking for SimulatedUserTracking {
    fn idle_until_input(&self) -> color_eyre::Result<Duration> {
        let start = SimulatedInstant::now();
        loop {
            if params().moving {
                break;
            } else {
                catch_up_sim_time();
                park_if_time_caught_up();
            }
        }

        Ok(start.elapsed())
    }

    fn reset_or_time_for_break<T: Time>(
        &mut self,
        &ScheduledBreak {
            next_at: next_break,
            between,
            ..
        }: &ScheduledBreak<T>,
    ) -> color_eyre::Result<ResetOrBreak> {
        let mut idle_since: Option<SimulatedInstant> = None;

        while next_break.in_the_future() {
            match was_idle_or_timeout(next_break) {
                IdleOrTimeout::Timeout => (),
                IdleOrTimeout::IdleFor(idle_time) if idle_time > *between => {
                    return Ok(ResetOrBreak::Reset { idle_time });
                }
                IdleOrTimeout::IdleFor(_) => (),
            }
        }
        Ok(ResetOrBreak::Break)
    }
}

fn was_idle_or_timeout<T: Time>(timeout: &T) -> IdleOrTimeout {
    loop {
        if params().moving {
            if params().time_still_to_run() > timeout.duration_until() {
                advance_sim_time_by(timeout.duration_until());
                return IdleOrTimeout::Timeout;
            } else {
                catch_up_sim_time();
                park();
            }
        } else {
            let idle = params().time_still_to_run();
            catch_up_sim_time();
            return IdleOrTimeout::IdleFor(idle);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SimulatedInstant(Duration);

impl Add<Duration> for SimulatedInstant {
    type Output = SimulatedInstant;

    fn add(self, rhs: Duration) -> Self::Output {
        SimulatedInstant(self.0 + rhs)
    }
}

impl AddAssign<Duration> for SimulatedInstant {
    fn add_assign(&mut self, rhs: Duration) {
        *self = *self + rhs
    }
}

impl Time for SimulatedInstant {
    fn now() -> Self {
        TIME.get()
    }
    fn elapsed(&self) -> Duration {
        let now = Self::now();
        now.0.saturating_sub(self.0)
    }
    fn saturating_duration_since(&self, earlier: Self) -> Duration {
        self.0.saturating_sub(earlier.0)
    }

    fn sleep(mut dur: Duration) {
        loop {
            park_if_time_caught_up();
            if params().instant_to_run_till > SimulatedInstant::now() + dur {
                advance_sim_time_by(dur);
                return;
            } else {
                dur -= params().time_still_to_run();
                advance_sim_time_by(params().time_still_to_run());
            }
        }
    }
}

pub fn park_if_time_caught_up() {
    if params().instant_to_run_till >= TIME.get() {
        params().parked = true;
        std::thread::park();
    }
}

fn advance_sim_time_by(dur: Duration) {
    TIME.set(SimulatedInstant::now() + dur)
}

fn catch_up_sim_time() {
    TIME.set(params().instant_to_run_till)
}

pub fn start_moving() {
    eprintln!("start moving");
    params().moving = true
}

pub fn stop_moving() {
    eprintln!("stop moving");
    params().moving = false
}

pub fn spawn_in_mocked_environment(f: impl FnOnce() + Send + 'static) {
    let now = SimulatedInstant(Duration::ZERO);
    let params = Box::leak(Box::new(Mutex::new(TestParams::new())));
    params
        .get_mut()
        .expect("no prior poison")
        .instant_to_run_till = now;
    let params = &*params;
    THREAD.set(Some(std::thread::spawn(move || {
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
    eprintln!("advancing time");
    params().instant_to_run_till += dur;
    dbg!(params().instant_to_run_till);
    unpark();

    let run_stated = Instant::now();
    while !params().parked {
        thread::sleep(Duration::from_millis(1));

        if run_stated.elapsed() > Duration::from_millis(10) {
            panic!(
                "Simulation took too long park, it is probably hanging somewhere"
            );
        }
    }
}
