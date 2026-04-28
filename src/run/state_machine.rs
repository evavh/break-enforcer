#[cfg(test)]
mod mocking;

use core::fmt;
use std::ops::{Add, AddAssign};
use std::time::Duration;

use crate::run::ScheduledBreak;

use super::{Break, NewState, ResetOrBreak};
use color_eyre::Result;

pub trait UserTracking {
    fn idle_until_input(&self) -> color_eyre::Result<Duration>;
    fn reset_or_time_for_break<T: Time>(
        &mut self,
        closest_break: &crate::run::ScheduledBreak<T>,
    ) -> color_eyre::Result<ResetOrBreak>;
}
pub trait Devices {
    type LockGuard;
    fn lock(&self) -> color_eyre::Result<Vec<Self::LockGuard>>;
}

pub trait Time:
    AddAssign<Duration> + Add<Duration> + Sized + Ord + Clone + fmt::Debug + Copy
{
    fn now() -> Self;
    #[allow(unused)]
    fn elapsed(&self) -> Duration;
    fn saturating_duration_since(&self, earlier: Self) -> Duration;
    fn sleep(dur: Duration);
}

impl Time for std::time::Instant {
    fn now() -> Self {
        std::time::Instant::now()
    }
    fn elapsed(&self) -> Duration {
        self.elapsed()
    }
    fn saturating_duration_since(&self, earlier: Self) -> Duration {
        self.saturating_duration_since(earlier)
    }
    fn sleep(dur: Duration) {
        std::thread::sleep(dur);
    }
}

pub fn run<T: Time<Output = T>>(
    mut tracking: impl UserTracking,
    devices: impl Devices,
    breaks: &[Break],
    mut handle_state_change: impl FnMut(NewState<T>),
) -> Result<()> {
    let mut breaks: Vec<_> = breaks
        .iter()
        .map(|b| ScheduledBreak {
            duration: b.duration,
            between: b.between,
            next_at: T::now() + b.between,
        })
        .collect();

    loop {
        // Idle
        handle_state_change(NewState::Idle);
        let idle_time = tracking.idle_until_input()?;
        breaks.iter_mut().for_each(|b| b.next_at += idle_time);

        // Running
        let closest_break = breaks
            .iter()
            .min_by_key(|b| b.next_at) // closest break
            .expect("breaks may not be empty")
            .clone();
        handle_state_change(NewState::Running {
            next: closest_break.clone(),
        });
        dbg!();
        if let ResetOrBreak::Reset { idle_time } =
            dbg!(tracking.reset_or_time_for_break(&closest_break))?
        {
            breaks.iter_mut().for_each(|b| b.next_at += idle_time);
            continue;
        }
        dbg!();

        // Break
        handle_state_change(NewState::Break {
            current: closest_break.clone(),
        });
        let _guard = devices.lock();
        dbg!();
        T::sleep(closest_break.duration - idle_time);
        dbg!();
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::thread;
    use std::time::Duration;

    use super::*;
    use crate::run::state_machine::mocking::{
        SimulatedInstant, advance_time_by, params, spawn_in_mocked_environment,
        start_moving, stop_moving, unpark,
    };
    use mocking::{FakeDevices, SimulatedUserTracking};

    fn breaks() -> Vec<Break> {
        vec![
            Break {
                duration: Duration::from_secs(30),
                between: Duration::from_secs(60),
            },
            Break {
                duration: Duration::from_secs(120),
                between: Duration::from_secs(120),
            },
        ]
    }

    thread_local! {
        pub static STATE: Cell<Option<NewState>> = const { Cell::new(None) };
    }

    #[track_caller]
    fn assert_state(assertion: impl Fn(NewState<SimulatedInstant>) -> bool) {
        let started = std::time::Instant::now();

        while !params().parked {
            thread::sleep(Duration::from_millis(1));
            if started.elapsed() > Duration::from_millis(10) {
                panic!("state machine thread did not park");
            }
        }

        while params().state.is_none() {
            thread::sleep(Duration::from_millis(1));
            if started.elapsed() > Duration::from_millis(10) {
                panic!("state machine did not reach first state");
            }
        }
        assert!(
            assertion(params().state.unwrap()),
            "state: {:?}",
            params().state
        );
    }

    #[track_caller]
    fn assert_idle() {
        assert_state(|curr| matches!(curr, NewState::Idle));
    }

    #[track_caller]
    fn assert_running() {
        assert_state(|curr| matches!(curr, NewState::Running { .. }));
    }

    #[track_caller]
    fn assert_break() {
        assert_state(|curr| matches!(curr, NewState::Break { .. }));
    }

    fn set_state(state: NewState<SimulatedInstant>) {
        params().state = Some(state);
    }

    trait BreaksExt {
        fn almost_reset(&self) -> Duration;
        fn just_after_reset(&self) -> Duration;
        fn shortest_run(&self) -> Duration;
    }

    impl BreaksExt for Vec<Break> {
        fn almost_reset(&self) -> Duration {
            self.iter()
                .map(|Break { duration, .. }| *duration)
                .min()
                .unwrap_or(Duration::ZERO)
                .saturating_sub(Duration::from_millis(1))
        }
        fn shortest_run(&self) -> Duration {
            self.iter()
                .map(|Break { between, .. }| *between)
                .min()
                .unwrap_or(Duration::ZERO)
        }
        fn just_after_reset(&self) -> Duration {
            self.almost_reset() + Duration::from_millis(2)
        }
    }

    #[test]
    fn start_idle_then_running() {
        spawn_in_mocked_environment(|| {
            run::<SimulatedInstant>(
                SimulatedUserTracking,
                FakeDevices,
                &mut breaks(),
                set_state,
            )
            .unwrap()
        });

        unpark();
        assert_idle();
        start_moving();
        advance_time_by(breaks().almost_reset());
        assert_running();
    }

    #[test]
    fn reset_if_no_movement() {
        spawn_in_mocked_environment(|| {
            run::<SimulatedInstant>(
                SimulatedUserTracking,
                FakeDevices,
                &mut breaks(),
                set_state,
            )
            .unwrap()
        });

        unpark();
        assert_idle();
        start_moving();
        advance_time_by(Duration::from_millis(1));
        assert_running();
        advance_time_by(breaks().just_after_reset());
        assert_idle();
    }

    #[test]
    fn idle_for_longer_then_break() {
        spawn_in_mocked_environment(|| {
            run::<SimulatedInstant>(
                SimulatedUserTracking,
                FakeDevices,
                &mut breaks(),
                set_state,
            )
            .unwrap()
        });

        stop_moving();
        advance_time_by(Duration::from_millis(1));
        start_moving();
        advance_time_by(Duration::from_millis(1));
        assert_running();

        advance_time_by(breaks().shortest_run());
        advance_time_by(Duration::from_millis(1));
        assert_break();
    }
}
