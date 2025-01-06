use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use tracing::debug;

mod tcp_api_config;
use tcp_api_config::PORTS;
use tcp_api_config::STOP_BYTE;

pub struct Api {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
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
}

impl Api {
    pub fn new() -> Result<Self, Error> {
        let mut conn = None;

        for port in PORTS {
            let addr = SocketAddr::from(([127, 0, 0, 1], port));
            match TcpStream::connect(addr) {
                Ok(c) => {
                    debug!("connected to break-enforcer service on port: {port}");
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
        let packet = String::from_utf8(packet.to_vec()).map_err(Error::CorruptResponse)?;

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
        let status = String::from_utf8(packet.to_vec()).map_err(Error::CorruptResponse)?;
        Ok(status)
    }
}
