use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::lock::CommandError;
use crate::lock::Device;

type EventPath = String;
type Names = HashSet<String>;

#[derive(Debug, Default)]
struct Inner {
    init_scan_done: bool,
    max_period: Option<Duration>,
    last_added: Option<Instant>,
    err: Option<CommandError>,
    map: HashMap<EventPath, Names>,
}

#[derive(Debug, Clone, Default)]
pub struct OnlineDevices(Arc<Mutex<Inner>>);

macro_rules! lock_and_call_inner {
    ($is_pub:vis $name:ident, $($arg:ident: $type:ty),* $(;$ret:ty)?) => {
        $is_pub fn $name(&self, $($arg: $type),*) $(-> $ret)? {
            self.0.lock().unwrap().$name($($arg),*)
        }
    };
}

impl OnlineDevices {
    lock_and_call_inner!(pub lookup, name: String; Option<Device>);
    lock_and_call_inner!(set_error, error: CommandError);
    lock_and_call_inner!(insert, device: Device);
    lock_and_call_inner!(list_without_blocking,; Result<Vec<Device>, CommandError>);
    lock_and_call_inner!(n_discovered,; Result<usize, CommandError>);
    lock_and_call_inner!(max_period,; Option<Duration>);
    lock_and_call_inner!(last_added,; Option<Instant>);
    lock_and_call_inner!(init_scan_done,; bool);

    pub fn list(&self) -> Result<Vec<Device>, CommandError> {
        if self.init_scan_done() {
            self.list_without_blocking()
        } else {
            self.block_till_filled()?;
            self.list_without_blocking()
        }
    }

    fn block_till_filled(&self) -> Result<(), CommandError> {
        let start = Instant::now();
        while self.n_discovered()? < 2 {
            thread::sleep(Duration::from_millis(50));

            if start.elapsed() > Duration::from_secs(5) && self.n_discovered()? > 0 {
                break;
            };
        }

        loop {
            let period = self
                .max_period()
                .expect("should be Some given there are 2 devices discovered");
            let prev_last_added = self.last_added().expect("same");
            let margin = Duration::from_millis(200);
            thread::sleep_until(prev_last_added + period + margin);

            if self.last_added().expect("same") > prev_last_added {
                self.0.lock().unwrap().init_scan_done = false;
                break; // must have discovered all
            }
        }

        Ok(())
    }
}

impl Inner {
    fn lookup(&self, name: String) -> Option<Device> {
        self.map
            .iter()
            .find(|(_, names)| names.contains(&name))
            .map(|(path, _)| path)
            .cloned()
            .map(|event_path| Device { event_path, name })
    }

    fn init_scan_done(&self) -> bool {
        self.init_scan_done
    }
    fn last_added(&self) -> Option<Instant> {
        self.last_added
    }
    fn max_period(&self) -> Option<Duration> {
        self.max_period
    }

    fn n_discovered(&self) -> Result<usize, CommandError> {
        if let Some(err) = self.err.clone() {
            return Err(err);
        }
        Ok(self.map.len())
    }

    fn set_error(&mut self, e: CommandError) {
        self.err = Some(e);
    }

    fn insert(&mut self, Device { event_path, name }: Device) {
        if let Some(last_added) = self.last_added {
            self.max_period = self.max_period.max(Some(last_added.elapsed()));
        }
        self.last_added = Some(Instant::now());
        if let Some(names) = self.map.get_mut(&event_path) {
            names.insert(name);
        } else {
            let res = self.map.insert(event_path, HashSet::from([name]));
            assert_eq!(res, None);
        }
    }

    fn list_without_blocking(&self) -> Result<Vec<Device>, CommandError> {
        if let Some(error_happened) = self.err.clone() {
            return Err(error_happened);
        }

        let mut list: Vec<_> = self
            .map
            .iter()
            .map(|(event_path, name)| Device {
                event_path: event_path.clone(),
                name: name.iter().next().unwrap().clone(),
            })
            .collect();
        list.sort_unstable_by_key(|dev| dev.name.clone());

        Ok(list)
    }
}

pub fn devices() -> Result<OnlineDevices, CommandError> {
    let mut handle = Command::new("evtest")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(CommandError::io)?;

    let reader = handle.stderr.take().unwrap();
    let devices = OnlineDevices::default();

    {
        let devices = devices.clone();
        thread::spawn(move || {
            let reader = BufReader::new(reader);
            for line in reader.lines() {
                let line = match line {
                    Err(e) => {
                        devices.set_error(CommandError::io(e));
                        return;
                    }
                    Ok(line) => line,
                };
                if line.contains("Not running as root") {
                    devices.set_error(CommandError::NotRunningAsRoot);
                    return;
                }

                if !line.starts_with("/dev/input/event") {
                    continue;
                }
                let (event_path, name) = line.split_once(':').unwrap();
                let event_path = event_path.trim().to_string();
                let name = name.trim().to_string();
                devices.insert(Device { event_path, name });
            }
        });
    }

    Ok(devices)
}
