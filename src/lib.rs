use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

mod tcp_api_config;
use tcp_api_config::PORTS;
use tcp_api_config::STOP_BYTE;

pub struct Api {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
}

#[derive(Default)]
pub struct ReconnectingApi(ApiState);

pub struct ReconnectingSubscriber(ApiState);

#[derive(Default)]
enum ApiState {
    #[default]
    Disconnected,
    Connected(Api),
}

impl ApiState {
    fn call_with_connected<T>(
        &mut self,
        call: impl Fn(&mut Api) -> Result<T, Error>,
        post_connect: impl Fn(&mut Api) -> Result<(), Error>,
    ) -> Result<T, Error> {
        let placeholder = ApiState::default();
        let owned_self = core::mem::replace(self, placeholder);

        let mut api = match owned_self {
            ApiState::Disconnected => {
                let mut api = Api::new()?;
                post_connect(&mut api)?;
                api
            }
            ApiState::Connected(api) => api,
        };

        match call(&mut api) {
            Ok(status) => {
                *self = ApiState::Connected(api);
                Ok(status)
            }
            Err(e) => {
                *self = ApiState::Disconnected;
                Err(e)
            }
        }
    }
}

impl ReconnectingApi {
    pub fn new() -> Self {
        Self(ApiState::Disconnected)
    }

    pub fn subscribe(mut self) -> ReconnectingSubscriber {
        let _ = self.0.call_with_connected(Api::subscribe, |_| Ok(()));
        ReconnectingSubscriber(self.0)
    }

    pub fn status(&mut self) -> Result<String, Error> {
        self.0.call_with_connected(Api::status, |_| Ok(()))
    }
}

impl ReconnectingSubscriber {
    pub fn recv_update(&mut self) -> StateUpdate {
        loop {
            match self
                .0
                .call_with_connected(Api::block_till_update, Api::subscribe)
            {
                Ok(update) => return update,
                Err(err) => {
                    warn!("Error while waiting for update: {err}");
                    thread::sleep(Duration::from_secs(5));
                }
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum StateUpdate {
    ParameterChange {
        break_duration: Duration,
        work_duration: Duration,
    },
    BreakStarted,
    BreakEnded,
    WentIdle,
    Reset,
    LongReset,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Could not connect on any of the ports the api server listens on")]
    CouldNotConnect,
    #[error("Error writing request")]
    WritingRequest(#[source] std::io::Error),
    #[error("Error while reading response")]
    ReadingResponse(#[source] std::io::Error),
    #[error("The response is not valid utf8")]
    CorruptResponse(#[source] std::string::FromUtf8Error),
    #[error("The api server closed the connection, did it halt?")]
    ConnectionClosed,
    #[error("The response should be a number, could not be parsed as one, response: {packet}")]
    IncorrectResponse {
        packet: String,
        #[source]
        error: std::num::ParseIntError,
    },
    #[error("Could not Deserialize status")]
    DeserializeStatus {
        packet: String,
        #[source]
        error: ron::error::SpannedError,
    },
}

impl Api {
    pub fn new() -> Result<Self, Error> {
        let mut conn = None;

        for port in PORTS {
            let addr = SocketAddr::from(([127, 0, 0, 1], port));
            match TcpStream::connect(addr) {
                Ok(c) => {
                    debug!(
                        "connected to break-enforcer service on port: {port}"
                    );
                    conn = Some(c);
                    break;
                }
                Err(e) => {
                    debug!(
                        "error connecting to api on port: {port}. Error: {e}. Trying another port"
                    );
                }
            };
        }

        let Some(conn) = conn else {
            return Err(Error::CouldNotConnect);
        };

        let writer = conn.try_clone().expect("tcp stream clone failed");
        let reader = BufReader::new(conn);

        Ok(Self { reader, writer })
    }

    pub fn idle_since(&mut self) -> Result<Duration, Error> {
        let mut request = b"idle_since".to_vec();
        request.push(STOP_BYTE);
        self.writer
            .write_all(&request)
            .map_err(Error::WritingRequest)?;

        let mut buf = Vec::new();
        let n_read = self
            .reader
            .read_until(STOP_BYTE, &mut buf)
            .map_err(Error::ReadingResponse)?;

        if n_read == 0 {
            return Err(Error::ConnectionClosed);
        }

        let packet = &buf[..(n_read - 1)]; // leave off STOP_BYTE
        let packet = String::from_utf8(packet.to_vec())
            .map_err(Error::CorruptResponse)?;

        let seconds_idle = packet
            .as_str()
            .parse::<u64>()
            .map_err(|error| Error::IncorrectResponse { packet, error })?;

        Ok(Duration::from_secs(seconds_idle))
    }

    pub fn status(&mut self) -> Result<String, Error> {
        let mut request = b"status_msg".to_vec();
        request.push(STOP_BYTE);
        self.writer
            .write_all(&request)
            .map_err(Error::WritingRequest)?;

        let mut buf = Vec::new();
        let n_read = self
            .reader
            .read_until(STOP_BYTE, &mut buf)
            .map_err(Error::ReadingResponse)?;

        if n_read == 0 {
            return Err(Error::ConnectionClosed);
        }

        let packet = &buf[..(n_read - 1)]; // leave off STOP_BYTE
        let status = String::from_utf8(packet.to_vec())
            .map_err(Error::CorruptResponse)?;
        Ok(status)
    }

    pub fn subscribe(&mut self) -> Result<(), Error> {
        let mut request = b"subscribe_to_state_changes".to_vec();
        request.push(STOP_BYTE);
        self.writer
            .write_all(&request)
            .map_err(Error::WritingRequest)?;
        Ok(())
    }

    pub fn block_till_update(&mut self) -> Result<StateUpdate, Error> {
        let mut buf = Vec::new();
        let n_read = self
            .reader
            .read_until(STOP_BYTE, &mut buf)
            .map_err(Error::ReadingResponse)?;

        if n_read == 0 {
            return Err(Error::ConnectionClosed);
        }

        let packet = &buf[..(n_read - 1)]; // leave off STOP_BYTE
        let status = String::from_utf8(packet.to_vec())
            .map_err(Error::CorruptResponse)?;
        let status = ron::from_str(&status).map_err(|error| {
            Error::DeserializeStatus {
                packet: status,
                error,
            }
        })?;
        Ok(status)
    }
}
