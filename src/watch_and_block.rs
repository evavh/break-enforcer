use core::fmt;
use std::collections::{HashMap, HashSet};
use std::io::ErrorKind;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use std::{fs, thread};

use base64::{engine::general_purpose, Engine as _};
use color_eyre::eyre::Context;
use color_eyre::{Result, Section};
use inotify::{EventMask, Inotify, WatchMask};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, warn};

use crate::check_inputs::device_removed;
use crate::config::InputFilter;

struct Device {
    locked: bool,
    raw_dev: evdev::Device,
}

fn device_name(device: &evdev::Device) -> String {
    let default = || {
        let id = InputId::from(device.input_id());
        format!("Unknown device, id: {id}")
    };
    device
        .name()
        .or(device.unique_name())
        .map_or_else(default, String::from)
}

impl Device {
    fn name(&self) -> String {
        device_name(&self.raw_dev)
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Hash)]
pub struct InputId {
    vendor: u16,
    product: u16,
    version: u16,
}

impl fmt::Display for InputId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let data = [
            self.vendor.to_be_bytes(),
            self.product.to_be_bytes(),
            self.version.to_be_bytes(),
        ];

        let base64 = general_purpose::URL_SAFE_NO_PAD.encode(data.flatten());
        f.write_str(base64.as_str())
    }
}

impl From<evdev::InputId> for InputId {
    fn from(value: evdev::InputId) -> Self {
        Self {
            vendor: value.vendor(),
            product: value.product(),
            version: value.version(),
        }
    }
}

macro_rules! lock_and_call_inner {
    ($is_pub:vis $name:ident, $($arg:ident: $type:ty),* $(;$ret:ty)?) => {
        $is_pub fn $name(&self, $($arg: $type),*) $(-> $ret)? {
            self.inner.lock().unwrap().$name($($arg),*)
        }
    };
}

#[derive(Clone)]
pub struct OnlineDevices {
    tx: mpsc::Sender<Event>,
    inner: Arc<Mutex<Inner>>,
}

impl OnlineDevices {
    lock_and_call_inner!(pub list_inputs,; Result<Vec<BlockableInput>>);
    lock_and_call_inner!(insert, raw_dev: evdev::Device, event_path: PathBuf; bool);
    lock_and_call_inner!(remove, event_path: PathBuf);
    lock_and_call_inner!(lock_all_matching, id: &InputFilter; Result<()>);
    lock_and_call_inner!(unlock_all_matching, id: &InputFilter; Result<()>);

    /// will also ensure that if the device is connected before
    /// the lockguard is dropped that it is locked
    pub(crate) fn lock(&self, input: InputFilter) -> Result<LockGuard> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.tx
            .send(Event::LockRequested(input.clone(), tx))
            .expect("devices should never end/panic");

        let lock_res = rx.recv().expect("devices should never end/panic");
        lock_res.wrap_err("Could not lock device")?;

        Ok(LockGuard {
            filter: input,
            tx: self.tx.clone(),
            dropped: false,
        })
    }
}

enum Event {
    LockRequested(InputFilter, mpsc::Sender<Result<()>>),
    UnLockRequested(InputFilter, mpsc::Sender<Result<()>>),
    DevError(color_eyre::Result<()>),
    DevAdded(PathBuf),
    DevRemoved(PathBuf),
}

/// use `unlock` to re-enable the disabled input device
#[must_use]
pub struct LockGuard {
    filter: InputFilter,
    tx: mpsc::Sender<Event>,
    // skip backup unlock if user did things right
    dropped: bool,
}

impl LockGuard {
    pub(crate) fn unlock(mut self) -> Result<()> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.tx
            .send(Event::UnLockRequested(self.filter.clone(), tx))
            .expect("devices should never end/panic");

        rx.recv().expect("devices should never end/panic")?;
        self.dropped = true;
        Ok(())
    }
}

/// backup, user should call unlock!
impl Drop for LockGuard {
    fn drop(&mut self) {
        if self.dropped {
            return; // nothing to do
        }
        let (tx, _) = std::sync::mpsc::channel();
        let _do_not_panic_in_drop = self
            .tx
            .send(Event::UnLockRequested(self.filter.clone(), tx));
        eprintln!(
            "Should not drop LockGuard but instead destroy by calling unlock
            since drop can not return an error"
        );
    }
}

struct Inner {
    // multiple devices with the same id could have different
    // names due to manufacturer mistake
    // device serial could be duplicate due to manufacturer mistake
    id_to_devices: HashMap<InputId, HashMap<PathBuf, Device>>,
    status: Result<()>,
}

impl Inner {
    fn check_status(&mut self) -> Result<()> {
        if self.status.is_err() {
            // little dance to get ownership of the error
            let mut to_return = Ok(());
            std::mem::swap(&mut to_return, &mut self.status);
            // self.error is now Ok(())
            to_return
        } else {
            Ok(())
        }
    }

    /// if it was already present ignore
    fn insert(&mut self, raw_dev: evdev::Device, event_path: PathBuf) -> bool {
        let id = raw_dev.input_id().into();
        let device = Device {
            raw_dev,
            locked: false,
        };
        if let Some(in_map) = self.id_to_devices.get_mut(&id) {
            let existing = in_map.insert(event_path, device);
            existing.is_none() // is_new
        } else {
            self.id_to_devices
                .insert(id, HashMap::from([(event_path, device)]));
            true
        }
    }

    fn remove(&mut self, event_path: PathBuf) {
        let mut removed = Vec::new();
        if let Some(empty_after_remove) = self
            .id_to_devices
            .iter()
            .find(|(_, map)| {
                map.len() == 1 && *map.keys().next().expect("len is one") == event_path
            })
            .map(|(id, _)| id)
            .copied()
        {
            self.id_to_devices
                .remove(&empty_after_remove)
                .expect("just found")
                .values()
                .map(Device::name)
                .collect_into(&mut removed);
        }

        for (_, inputs) in self.id_to_devices.iter_mut() {
            if let Some(device) = inputs.remove(&event_path) {
                removed.push(device.name());
            }
        }

        if removed.is_empty() {
            warn!(
                "Device disconnected but it wasnt registered, event_path: {}",
                event_path.display()
            );
        } else {
            debug!("Device(s) disconnected: {removed:?}");
        }
    }

    fn list_inputs(&mut self) -> Result<Vec<BlockableInput>> {
        self.check_status()?;

        Ok(self
            .id_to_devices
            .iter()
            .map(|(id, devices)| {
                let mut names: Vec<_> = devices.values().map(Device::name).collect();
                names.sort();
                BlockableInput { id: *id, names }
            })
            .collect())
    }

    fn unlock_all_matching(&mut self, filter: &InputFilter) -> Result<()> {
        self.check_status()?;
        let Some(to_lock) = self.id_to_devices.get_mut(&filter.id) else {
            return Ok(());
        };

        for device in to_lock
            .values_mut()
            .filter(|device| device.locked)
            .filter(|device| filter.names.contains(&device.name()))
        {
            match device.raw_dev.ungrab() {
                Ok(()) => {
                    debug!("Unlocked: {}", device.name());
                    device.locked = false;
                }
                Err(e) if device_removed(&e) => {
                    warn!(
                        "Could not unlock, device probably removed: {}",
                        device.name()
                    )
                }
                err @ Err(_) => {
                    return err
                        .wrap_err("Could not ungrab (release exclusive access) to device")
                        .with_note(|| format!("device name: {}", device.name()));
                }
            }
        }
        Ok(())
    }

    fn lock_all_matching(&mut self, filter: &InputFilter) -> Result<()> {
        self.check_status()?;
        let Some(to_lock) = self.id_to_devices.get_mut(&filter.id) else {
            return Ok(());
        };

        for device in to_lock
            .values_mut()
            .filter(|device| !device.locked)
            .filter(|device| filter.names.contains(&device.name()))
        {
            match device.raw_dev.grab() {
                Ok(()) => {
                    debug!("Locked: {}", device.name());
                    device.locked = true;
                }
                Err(e) if e.kind() == ErrorKind::ResourceBusy => {
                    warn!("Could not lock, device busy: {}", device.name())
                }
                Err(e) if device_removed(&e) => {
                    warn!("Could not lock, device probably removed: {}", device.name())
                }
                err @ Err(_) => {
                    return err
                        .wrap_err("Could not grab (acquire exclusive access) to device")
                        .with_note(|| format!("device name: {}", device.name()))
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct BlockableInput {
    pub id: InputId,
    pub names: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct NewInput {
    pub id: InputId,
    pub name: String,
    pub path: PathBuf,
}

pub fn devices() -> (OnlineDevices, Receiver<NewInput>) {
    let (order_tx, order_rx) = mpsc::channel();
    let mut online = OnlineDevices {
        tx: order_tx.clone(),
        inner: Arc::new(Mutex::new(Inner {
            status: Ok(()),
            id_to_devices: HashMap::new(),
        })),
    };

    let (new_dev_tx, new_dev_rx) = mpsc::channel();
    send_initial_devices(&mut online, &new_dev_tx);
    thread::spawn(move || {
        send_new_devices(&order_tx);
    });

    let mut locked = HashSet::new();
    let mut online2 = online.clone();
    thread::spawn(move || loop {
        match order_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Event::LockRequested(filter, answer)) => {
                let res = online2.lock_all_matching(&filter);
                locked.insert(filter);
                answer.send(res).expect("lock fn does not panic");
            }
            Ok(Event::UnLockRequested(filter, answer)) => {
                locked.remove(&filter);
                let res = online2.unlock_all_matching(&filter);
                answer.send(res).expect("unlock fn does not panic");
            }
            Ok(Event::DevAdded(event_path)) => {
                add_device(&mut online2, &new_dev_tx, event_path);
                for filter in &locked {
                    if let Err(e) = online2.lock_all_matching(filter) {
                        error!("Failed to lock devices matching filter, error: {e:?}");
                        online2.inner.lock().unwrap().status = Err(e);
                    }
                }
            }
            Ok(Event::DevRemoved(event_path)) => {
                online2.remove(event_path);
            }
            Ok(Event::DevError(error)) => {
                // next time online devices is queried it will report this error
                online2.inner.lock().unwrap().status = error;
            }

            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return,
        }
    });

    (online, new_dev_rx)
}

const DEV_DIR: &str = "/dev/input";
fn send_initial_devices(online: &mut OnlineDevices, new_dev_tx: &Sender<NewInput>) {
    for entry in fs::read_dir(DEV_DIR).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let fname = path.file_name().unwrap();
        // note, there are legacy events (mouse/js) these are
        // duplicates of the event<number> devices. Therefore we
        // do not add them.
        if fname.as_bytes().starts_with(b"event") {
            add_device(online, new_dev_tx, path);
        }
    }
}

type DeviceName = String;
fn add_device(
    online: &mut OnlineDevices,
    new_dev_tx: &Sender<NewInput>,
    event_path: PathBuf,
) -> Option<DeviceName> {
    let Ok(device) = evdev::Device::open(&event_path) else {
        warn!(
            "Could not open device at: {}, ignoring the device",
            event_path.display()
        );
        return None;
    };
    let id = InputId::from(device.input_id());
    let name = device_name(&device);
    let new = online.insert(device, event_path.clone());
    if new {
        new_dev_tx
            .send(NewInput {
                id,
                name: name.clone(),
                path: event_path,
            })
            .expect("watcher should never end and drop rx");
        debug!("added device: {}", name);
        Some(name)
    } else {
        debug!("device: {} is already tracked", name);
        None
    }
}

fn send_new_devices(tx: &Sender<Event>) {
    let mut inotify = Inotify::init().unwrap();
    let mut buffer = [0; 1024];

    inotify
        .watches()
        .add(DEV_DIR, WatchMask::CREATE | WatchMask::DELETE)
        .unwrap();

    loop {
        let events = match inotify.read_events_blocking(&mut buffer) {
            Err(err) => {
                let res = Err(err).wrap_err("inotify could not read events");
                tx.send(Event::DevError(res)).unwrap();
                return;
            }
            Ok(events) => events,
        };

        for event in events {
            let Some(file_name) = event.name else {
                continue;
            };
            // note, there are legacy events (mouse/js) these are
            // duplicates of the event<number> devices. Therefore we
            // do not respond to them.
            if !file_name.as_bytes().starts_with(b"event") {
                continue;
            }

            let path = PathBuf::from_str(DEV_DIR).unwrap().join(file_name);
            if event.mask.contains(EventMask::CREATE) {
                tx.send(Event::DevAdded(path.clone())).unwrap();
            } else if event.mask.contains(EventMask::DELETE) {
                tx.send(Event::DevRemoved(path.clone())).unwrap();
            }
        }
    }
}
