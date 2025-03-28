use std::{
    io::{Error, ErrorKind},
    net::SocketAddr,
    str::FromStr,
};

use anyhow::Result;
use byteorder::{ByteOrder, NetworkEndian};

use crypto_secretbox::XSalsa20Poly1305;
use tokio::{
    net::UdpSocket,
    sync::mpsc::{channel, Receiver, Sender},
};
use tracing::error;

use crate::crypto::EncryptionMode;

use super::VoiceError;

const SILENCE_FRAME: [u8; 3] = [0xf8, 0xff, 0xfe];

#[derive(Debug)]
pub enum UDPMessage {
    Silence,
    Audio(Vec<u8>),
}

pub struct VoiceUDP {
    ssrc: u32,
    local_addr: SocketAddr,
    mode: EncryptionMode,
    player_rx: Receiver<UDPMessage>,
    socket: UdpSocket,
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
    ) -> Result<(Self, Sender<UDPMessage>), VoiceError> {
        let (udp_tx, player_rx) = channel(1);
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(dest_ip).await?;
        let src_ip = Self::ip_discovery(&socket, ssrc).await?;

        Ok((
            Self {
                ssrc,
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
            let msg = match self.player_rx.recv().await {
                Some(msg) => msg,
                None => {
                    // Player and VoiceManager are dropped
                    return Ok(());
                }
            };

            let mut packet = vec![0u8; 12];
            packet[0] = 0x80;
            packet[1] = 0x78;
            NetworkEndian::write_u16(&mut packet[2..4], self.sequence);
            NetworkEndian::write_u32(&mut packet[4..8], self.timestamp);
            NetworkEndian::write_u32(&mut packet[8..12], self.ssrc);

            match msg {
                UDPMessage::Silence => {
                    let encrypted = self.mode.encrypt(&SILENCE_FRAME, &packet, &mut cipher)?;
                    if let Err(e) = self.socket.send(&encrypted).await {
                        error!("Packet dropped? {:?}", e);
                    }
                }
                UDPMessage::Audio(audio) => {
                    let encrypted = self.mode.encrypt(&audio, &packet, &mut cipher)?;
                    if let Err(e) = self.socket.send(&encrypted).await {
                        error!("Packet dropped? {:?}", e);
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

    async fn ip_discovery(socket: &UdpSocket, ssrc: u32) -> Result<SocketAddr, VoiceError> {
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
                        .expect("null byte should exist");
                    let ip = std::str::from_utf8(&discovery_buf[8..8 + null_byte_index])
                        .map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e))?;
                    let port = NetworkEndian::read_u16(&discovery_buf[discovery_buf.len() - 2..]);
                    let addr = SocketAddr::from_str(&format!("{}:{}", ip, port))
                        .map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e))?;
                    return Ok(addr);
                }
            }
        }
        Err(Error::new(ErrorKind::Other, "Failed to discover IP"))?
    }
}
