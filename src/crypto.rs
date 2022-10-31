use std::borrow::BorrowMut;

use anyhow::Result;
use byteorder::{ByteOrder, NetworkEndian};
use serde::{Deserialize, Serialize};

use rand::Rng;
use xsalsa20poly1305::{aead::Aead, KeyInit, XSalsa20Poly1305};

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
        data: &[u8],
        rtp_header: &[u8],
        secret_key: &[u8],
    ) -> Result<Vec<u8>> {
        let cipher = XSalsa20Poly1305::new(secret_key.into());
        match self {
            EncryptionMode::XSalsa20Poly1305Lite(ref mut nonce) => {
                let mut nonce_buf: [u8; 24] = [0u8; 24];
                NetworkEndian::write_u32(&mut nonce_buf[0..4], *nonce);
                *nonce += 1u32;
                match cipher.encrypt(&nonce_buf.into(), data) {
                    Ok(cipher_bytes) => Ok(cipher_bytes),
                    Err(e) => {
                        return Err(anyhow::anyhow!(
                            "XSalsa20Poly1305Lite encryption failed: {}",
                            e
                        ));
                    }
                }
            }
            EncryptionMode::XSalsa20Poly1305Suffix => {
                let mut rng = rand::thread_rng();
                let nonce = XSalsa20Poly1305::generate_nonce(&mut rng);
                match cipher.encrypt(&nonce, data) {
                    Ok(cipher_bytes) => Ok(cipher_bytes),
                    Err(e) => {
                        return Err(anyhow::anyhow!(
                            "XSalsa20Poly1305Suffix encryption failed: {}",
                            e
                        ));
                    }
                }
            }
            EncryptionMode::XSalsa20Poly1305 => {
                let mut nonce_buf = [0u8; 24];
                nonce_buf[..12].copy_from_slice(rtp_header);
                let cipher = XSalsa20Poly1305::new(secret_key.into());
                match cipher.encrypt(&nonce_buf.into(), data) {
                    Ok(cipher_bytes) => Ok(cipher_bytes),
                    Err(e) => {
                        return Err(anyhow::anyhow!(
                            "XSalsa20Poly1305 encryption failed: {}",
                            e
                        ));
                    }
                }
            }
        }
    }
}
