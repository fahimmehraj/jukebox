use std::{
    io::{Error, ErrorKind},
    net::SocketAddr,
    str::FromStr, sync::Arc,
};

use anyhow::Result;
use byteorder::{ByteOrder, NetworkEndian};

use crypto_secretbox::XSalsa20Poly1305;
use log::{debug, error, trace};
use tokio::{net::UdpSocket, sync::mpsc::{channel, Receiver, Sender}};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::crypto::EncryptionMode;

const SILENCE_FRAME: [u8; 3] = [0xf8, 0xff, 0xfe];


#[derive(Debug)]
pub enum UDPMessage {
    Silence,
    Audio(Vec<u8>),
}

pub struct VoiceUDP {
    ssrc: u32,
    remote_addr: SocketAddr,
    local_addr: SocketAddr,
    mode: EncryptionMode,
    player_rx: Receiver<UDPMessage>,
    socket: Arc<UdpSocket>,
    sequence: u16,
    timestamp: u32,
    cipher: Option<XSalsa20Poly1305>,
}

impl VoiceUDP {
    /// Initializes the UDP connection. The returned connection does not start out with
    /// a secret key.
    pub async fn connect(
        ssrc: u32,
        dest_ip: SocketAddr,
        mode: EncryptionMode,
    ) -> Result<(Self, Sender<UDPMessage>)> {
        let (udp_tx, player_rx) = channel(1);
        let socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
        socket.connect(dest_ip).await?;
        let src_ip = Self::ip_discovery(Arc::clone(&socket), ssrc).await?;

        Ok((
            Self {
                ssrc,
                remote_addr: dest_ip,
                local_addr: src_ip,
                mode,
                player_rx,
                socket,
                sequence: 0,
                timestamp: 0,
                cipher: None,
            },
            udp_tx,
        ))
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut cipher = self.cipher.take().ok_or(Error::new(
            ErrorKind::Other,
            "Cannot run UDP connection without a secret key",
        ))?;

        loop {
            let msg = self
                .player_rx
                .recv()
                .await
                .ok_or(Error::new(ErrorKind::Other, "Player channel closed"))?;

            let mut packet = vec![0u8; 12];
            packet[0] = 0x80;
            packet[1] = 0x78;
            NetworkEndian::write_u16(&mut packet[2..4], self.sequence);
            NetworkEndian::write_u32(&mut packet[4..8], self.timestamp);
            NetworkEndian::write_u32(&mut packet[8..12], self.ssrc);

            match msg {
                UDPMessage::Silence => {
                    let mut encrypted = self.mode.encrypt(&mut SILENCE_FRAME, &packet, &mut cipher)?;
                    if let Err(e) = self.socket.send(&encrypted).await {
                        error!("Packet dropped?");
                    }
                }
                UDPMessage::Audio(mut audio) => {
                    let encrypted = self.mode.encrypt(&mut audio, &packet, &mut cipher)?;
                    if let Err(e) = self.socket.send(&encrypted).await {
                        error!("Packet dropped?");
                    }
                }
            }
            self.sequence += 1;
            self.timestamp += 960;
        }
    }

    pub fn cipher_mut(&mut self) -> &mut Option<XSalsa20Poly1305> {
        &mut self.cipher
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn remote_addr(&self) -> SocketAddr {
        self.remote_addr
    }

    async fn ip_discovery(socket: Arc<UdpSocket>, ssrc: u32) -> Result<SocketAddr> {
        let mut discovery_buf = vec![0u8; 74];
        NetworkEndian::write_u16(&mut discovery_buf[0..2], 0x1);
        NetworkEndian::write_u16(&mut discovery_buf[2..4], 70);
        NetworkEndian::write_u32(&mut discovery_buf[4..8], ssrc);

        socket.send(&discovery_buf).await?;
        socket.recv(&mut discovery_buf).await?;
        if NetworkEndian::read_u16(&discovery_buf[0..2]) == 0x2 {
            if NetworkEndian::read_u16(&discovery_buf[2..4]) == 70 {
                if NetworkEndian::read_u32(&discovery_buf[4..8]) == ssrc {
                    let null_byte_index = &discovery_buf
                        .iter()
                        .skip(8)
                        .position(|&x| x == 0)
                        .expect("No null byte");
                    let ip = std::str::from_utf8(&discovery_buf[8..8 + null_byte_index])?;
                    let port = NetworkEndian::read_u16(&discovery_buf[discovery_buf.len() - 2..]);
                    return Ok(SocketAddr::from_str(&format!("{}:{}", ip, port))?);
                }
            }
        }
        Err(Error::new(ErrorKind::Other, "Failed to discover IP"))?
    }
}
