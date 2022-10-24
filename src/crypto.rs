use serde::{Deserialize, Serialize};


#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EncryptionMode {
    #[serde(rename = "xsalsa20_poly1305_lite")]
    XSalsa20Poly1305Lite,
    #[serde(rename = "xsalsa20_poly1305")]
    XSalsa20Poly1305Suffix,
    #[serde(rename = "xsalsa20_poly1305_suffix")]
    XSalsa20Poly1305,
}