#[cfg(test)]
mod mocking;

use std::time::Duration;

#[cfg(test)]
use crate::run::state_machine::mocking::{now, params, sleep};

#[cfg(not(test))]
use super::{RealDevices, RealUserTracking};
#[cfg(test)]
use mocking::{FakeDevices, FakeUserTracking};

use super::{Break, NewState, ResetOrBreak};
use color_eyre::Result;

#[cfg(not(test))]
fn sleep(dur: Duration) {
    std::thread::sleep(dur);
}

pub trait UserTracking {
    fn idle_until_input(&self) -> color_eyre::Result<Duration>;
    fn reset_or_time_for_break(
        &mut self,
        closest_break: &crate::run::Break,
    ) -> color_eyre::Result<ResetOrBreak>;
}
pub trait Devices {
    type LockGuard;
    fn lock(&self) -> color_eyre::Result<Vec<Self::LockGuard>>;
}

pub fn run(
    mut tracking: impl UserTracking,
    devices: impl Devices,
    breaks: &mut [Break],
    mut handle_state_change: impl FnMut(NewState),
) -> Result<()> {
    loop {
        dbg!();
        // Idle
        handle_state_change(NewState::Idle);
        dbg!();
        let idle_time = tracking.idle_until_input()?;
        dbg!();
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
        if let ResetOrBreak::Reset { idle_time } =
            tracking.reset_or_time_for_break(&closest_break)?
        {
            breaks.iter_mut().for_each(|b| b.next_at += idle_time);
            dbg!();
            continue;
        }

        // Break
        handle_state_change(NewState::Break {
            current: closest_break.clone(),
        });
        let _guard = devices.lock();
        sleep(closest_break.duration - idle_time);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::thread;
    use std::time::Duration;

    use super::*;
    use crate::run::state_machine::mocking::{
        advance_time_by, now, params, spawn_in_mocked_environment,
        start_moving, stop_moving, unpark,
    };

    fn breaks() -> Vec<Break> {
        vec![
            Break {
                duration: Duration::from_secs(30),
                between: Duration::from_secs(60),
                next_at: now(),
            },
            Break {
                duration: Duration::from_secs(120),
                between: Duration::from_secs(30),
                next_at: now(),
            },
        ]
    }

    thread_local! {
        pub static STATE: Cell<Option<NewState>> = const { Cell::new(None) };
    }

    #[track_caller]
    fn assert_state(assertion: impl Fn(NewState) -> bool) {
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

    fn set_state(state: NewState) {
        params().state = Some(state);
    }

    trait BreaksExt {
        fn almost_reset(&self) -> Duration;
        fn just_after_reset(&self) -> Duration;
    }

    impl BreaksExt for Vec<Break> {
        fn almost_reset(&self) -> Duration {
            self.iter()
                .map(|Break { duration, .. }| *duration)
                .min()
                .unwrap_or(Duration::ZERO)
                .saturating_sub(Duration::from_millis(1))
        }
        fn just_after_reset(&self) -> Duration {
            self.almost_reset() + Duration::from_millis(2)
        }
    }

    #[test]
    fn start_idle_then_running() {
        spawn_in_mocked_environment(|| {
            run(FakeUserTracking, FakeDevices, &mut breaks(), set_state)
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
            run(FakeUserTracking, FakeDevices, &mut breaks(), set_state)
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
            run(FakeUserTracking, FakeDevices, &mut breaks(), set_state)
                .unwrap()
        });

        dbg!();
        unpark();
        dbg!();
        start_moving();
        dbg!();
        advance_time_by(Duration::from_millis(1));
        dbg!();
        assert_running();

        stop_moving();
        for _ in 0..100 {
            advance_time_by(Duration::from_secs(2));
            assert_running();
        }
    }
}
