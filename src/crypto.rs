use anyhow::Result;
use byteorder::{ByteOrder, NetworkEndian};
use serde::{Deserialize, Serialize};

use crypto_secretbox::{aead::AeadMut, AeadCore, XSalsa20Poly1305};

#[derive(Clone, Copy, Serialize, Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EncryptionMode {
    #[serde(rename = "xsalsa20_poly1305")]
    XSalsa20Poly1305,
    #[serde(rename = "xsalsa20_poly1305_lite")]
    XSalsa20Poly1305Lite(#[serde(skip)] u32),
    #[serde(rename = "xsalsa20_poly1305_suffix")]
    XSalsa20Poly1305Suffix,
}

impl EncryptionMode {
    pub fn encrypt(
        &mut self,
        data: &[u8],
        rtp_header: &[u8],
        cipher: &mut XSalsa20Poly1305,
    ) -> Result<Vec<u8>> {
        match self {
            EncryptionMode::XSalsa20Poly1305Lite(ref mut nonce) => {
                let mut nonce_buf: [u8; 24] = [0u8; 24];
                NetworkEndian::write_u32(&mut nonce_buf[..4], *nonce);
                *nonce += 1u32;
                match cipher.encrypt(&nonce_buf.into(), data.as_ref()) {
                    Ok(ciphertext) => {
                        Ok([rtp_header, &ciphertext, &nonce_buf[..4]].concat())
                    }
                    Err(e) => return Err(anyhow::anyhow!(e)),
                }
            }
            EncryptionMode::XSalsa20Poly1305Suffix => {
                let nonce = XSalsa20Poly1305::generate_nonce(&mut rand::thread_rng());
                match cipher.encrypt(&nonce, data.as_ref()) {
                    Ok(ciphertext) => {
                        Ok([rtp_header, &ciphertext, nonce.as_ref()].concat())
                    }
                    Err(e) => return Err(anyhow::anyhow!(e)),
                }
            }
            EncryptionMode::XSalsa20Poly1305 => {
                let mut nonce_buf = [0u8; 24];
                nonce_buf[..12].copy_from_slice(rtp_header);
                match cipher.encrypt(&nonce_buf.into(), data.as_ref()) {
                    Ok(ciphertext) => Ok([rtp_header, &ciphertext].concat()),
                    Err(e) => Err(anyhow::anyhow!(e)),
                }
            }
        }
    }
}
