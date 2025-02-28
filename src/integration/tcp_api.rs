/// Simple ascii protocol over tcp, uses 0 bytes as packet framing
use std::io::{BufReader, ErrorKind, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Instant;

use break_enforcer::StateUpdate;
use color_eyre::eyre::{eyre, Context};
use color_eyre::{Result, Section};
use tracing::{debug, warn};

use crate::cli::RunArgs;
use crate::tcp_api_config::{PORTS, STOP_BYTE};

#[derive(Debug, Clone)]
pub(crate) struct Status {
    msg: Arc<Mutex<String>>,
    idle: Arc<Mutex<Instant>>,
    subscribers: Arc<Mutex<Vec<mpsc::Sender<StateUpdate>>>>,
}

impl Status {
    pub fn new(idle: Arc<Mutex<Instant>>) -> Self {
        Self {
            msg: Arc::new(Mutex::new(String::new())),
            idle,
            subscribers: Arc::new(Mutex::new(Vec::new())),
        }
    }
    pub fn msg(&self) -> String {
        self.msg
            .lock()
            .expect("Self::update_msg can not panic")
            .clone()
    }
    pub fn idle_since(&self) -> String {
        self.idle
            .lock()
            .expect("nothing can panic with lock held")
            .elapsed()
            .as_secs()
            .to_string()
    }

    pub(crate) fn update_msg(&self, new_status: &str) {
        let mut msg = self.msg.lock().expect("Self::msg can not panic");
        *msg = new_status.to_string();
    }

    pub(crate) fn update_subscribers(&self, just_enterd: &super::State) {
        let update = just_enterd.state_update();
        for sub in self
            .subscribers
            .lock()
            .expect("subscribe() should never panic")
            .iter()
        {
            // subscribers unsubscribing is not a reason to panic
            let _ = sub.send(update.clone());
        }
    }

    fn subscribe(&self, args: &RunArgs) -> mpsc::Receiver<StateUpdate> {
        let (tx, rx) = mpsc::channel();
        tx.send(StateUpdate::ParameterChange {
            break_duration: args.break_duration,
            work_duration: args.work_duration,
        })
        .expect("rx still in scope");
        self.subscribers
            .lock()
            .expect("update_subscribers should never panic")
            .push(tx);
        rx
    }
}

pub(crate) fn maintain(status: Status, args: RunArgs) -> Result<()> {
    let args = Arc::new(args);
    let mut listener = None;

    for port in PORTS {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        match TcpListener::bind(addr) {
            Ok(l) => {
                listener = Some(l);
                break;
            }
            Err(e) if e.kind() == ErrorKind::AddrInUse => {
                continue;
            }
            Err(other) => {
                return Err(other).wrap_err("Could not start listening")
            }
        };
    }

    let Some(listener) = listener else {
        return Err(eyre!(
            "Could not find a suitable port after trying multiple options"
        ));
    };

    for res in listener.incoming() {
        debug!("accepted api connection");
        let conn = match res {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed incoming connection: {e}");
                continue;
            }
        };

        let status = status.clone();
        let args = args.clone();
        thread::spawn(|| {
            if let Err(error) = handle_conn(conn, status, args) {
                warn!("ran into error handling API client: {error}");
            }
        });
    }

    Ok(())
}

fn handle_conn(
    conn: std::net::TcpStream,
    status: Status,
    args: Arc<RunArgs>,
) -> Result<()> {
    use std::io::BufRead;

    let mut writer = conn.try_clone().expect("tcp stream clone failed");
    let mut reader = BufReader::new(conn);
    let mut buf = vec![];

    loop {
        let n_read = reader.read_until(STOP_BYTE, &mut buf)?;
        if n_read == 0 {
            debug!("api client disconnected");
            return Ok(());
        }

        let packet = &buf[..(n_read - 1)]; // leave off STOP_BYTE
        let packet = String::from_utf8(packet.to_vec())
            .wrap_err("packet must consist of valid utf8")
            .with_note(|| format!("got bytes: {packet:?})"))?;

        match packet.as_str() {
            "status_msg" => {
                writer
                    .write_all(status.msg().as_bytes())
                    .wrap_err("Could not write status msg to tcpstream")?;
                writer
                    .write_all(&[STOP_BYTE])
                    .wrap_err("Could not write status msg to tcpstream")?;
            }
            "idle_since" => {
                writer
                    .write_all(status.idle_since().as_bytes())
                    .wrap_err("Could not write active or not to tcpstream")?;
                writer
                    .write_all(&[STOP_BYTE])
                    .wrap_err("Could not write active or not to tcpstream")?;
            }
            "subscribe_to_state_changes" => {
                handle_subscriber(&status, &args, &mut writer)?
            }
            _ => {
                debug!("packet: '{packet}'");
                return Err(eyre!(
                    "got unexpected packet/api request, disconnecting"
                ));
            }
        }
    }
}

fn handle_subscriber(
    status: &Status,
    args: &RunArgs,
    writer: &mut std::net::TcpStream,
) -> Result<(), color_eyre::eyre::Error> {
    let sub = status.subscribe(args);
    loop {
        let update = sub
            .recv()
            .expect("Should only be removed after we drop it here");
        writer
            .write_all(
                ron::to_string(&update)
                    .expect("serializing should not fail")
                    .as_bytes(),
            )
            .wrap_err("Could not write update to tcpstream")?;
        writer
            .write_all(&[STOP_BYTE])
            .wrap_err("Could not write update to tcpstream")?;
    }
}
