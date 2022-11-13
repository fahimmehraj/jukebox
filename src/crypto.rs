use anyhow::Result;
use byteorder::{ByteOrder, NetworkEndian};
use serde::{Deserialize, Serialize};

use xsalsa20poly1305::{
    AeadInPlace, XSalsa20Poly1305, TAG_SIZE, NONCE_SIZE,
};

#[derive(Clone, Copy, Serialize, Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EncryptionMode {
    #[serde(rename = "xsalsa20_poly1305_lite")]
    XSalsa20Poly1305Lite(#[serde(skip)] u32),
    #[serde(rename = "xsalsa20_poly1305")]
    XSalsa20Poly1305Suffix,
    #[serde(rename = "xsalsa20_poly1305_suffix")]
    XSalsa20Poly1305,
}

impl EncryptionMode {
    pub fn encrypt(
        &mut self,
        mut data: &mut [u8],
        rtp_header: &[u8],
        cipher: &XSalsa20Poly1305,
    ) -> Result<Vec<u8>> {
        let mut packet = vec![0; data.len() + 12 + TAG_SIZE];
        packet[..12].copy_from_slice(rtp_header);
        match self {
            EncryptionMode::XSalsa20Poly1305Lite(ref mut nonce) => {
                let mut nonce_buf: [u8; 24] = [0u8; 24];
                NetworkEndian::write_u32(&mut nonce_buf[0..4], *nonce);
                *nonce += 1u32;
                match cipher.encrypt_in_place_detached(&nonce_buf.into(), b"", &mut data) {
                    Ok(tag) => {
                        packet[12..12 + TAG_SIZE].copy_from_slice(&tag);
                        packet[12 + TAG_SIZE..12 + TAG_SIZE + data.len()].copy_from_slice(data);
                        packet.extend_from_slice(&nonce_buf[0..4]);
                        return Ok(packet);
                    }
                    Err(e) => return Err(anyhow::anyhow!(e)),
                }
            }
            EncryptionMode::XSalsa20Poly1305Suffix => {
                let nonce = XSalsa20Poly1305::generate_nonce(&mut rand::thread_rng());
                match cipher.encrypt_in_place_detached(&nonce, b"", &mut data) {
                    Ok(tag) => {
                        packet[12..12 + TAG_SIZE].copy_from_slice(&tag);
                        packet[12 + TAG_SIZE..12 + TAG_SIZE + data.len()].copy_from_slice(data);
                        packet.extend_from_slice(&nonce);
                        return Ok(packet);
                    }
                    Err(e) => return Err(anyhow::anyhow!(e)),
                }
            }
            EncryptionMode::XSalsa20Poly1305 => {
                let mut nonce_buf = [0u8; 24];
                nonce_buf[..12].copy_from_slice(rtp_header);
                match cipher.encrypt_in_place_detached(&nonce_buf.into(), b"", &mut data) {
                    Ok(tag) => {
                        packet[12..12 + TAG_SIZE].copy_from_slice(&tag);
                        packet[12 + TAG_SIZE..12 + TAG_SIZE + data.len()].copy_from_slice(data);
                        packet.extend_from_slice(&nonce_buf);
                        return Ok(packet);
                    }
                    Err(e) => return Err(anyhow::anyhow!(e)),
                }
            }
        }
    }
}
