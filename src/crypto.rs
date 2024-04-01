use hex::{self, ToHex};
use rsa::{
    pkcs1::{EncodeRsaPrivateKey as _, EncodeRsaPublicKey as _},
    sha2::Sha256,
    Pkcs1v15Sign, RsaPrivateKey, RsaPublicKey,
};
use serde::{Deserialize, Serialize};
use std::{
    fmt::{self, Debug, Formatter},
    iter::FromIterator,
};

const KEY_HEX_LEN: usize = 8;

// private key

const PRIVATE_PKCS1_DER_HEADER_LEN: usize = 12;

#[derive(Clone, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct PrivateKey(RsaPrivateKey);

impl PrivateKey {
    pub fn to_publ_key(&self) -> PublicKey {
        PublicKey::from(self.0.to_public_key())
    }

    pub fn to_der(&self) -> Vec<u8> {
        self.0.to_pkcs1_der().unwrap().as_bytes().to_vec()
    }

    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        self.0.sign(Pkcs1v15Sign::new::<Sha256>(), message).unwrap()
    }
}

impl From<RsaPrivateKey> for PrivateKey {
    fn from(priv_key: RsaPrivateKey) -> Self {
        Self(priv_key)
    }
}

impl ToHex for PrivateKey {
    fn encode_hex<T: FromIterator<char>>(&self) -> T {
        let start = PRIVATE_PKCS1_DER_HEADER_LEN;
        let end = start + KEY_HEX_LEN;

        // encode only the key itself, not the DER header
        let der = self.0.to_pkcs1_der().unwrap();
        let key = &der.as_bytes()[start..end];
        key.encode_hex()
    }

    fn encode_hex_upper<T: FromIterator<char>>(&self) -> T {
        let start = PRIVATE_PKCS1_DER_HEADER_LEN;
        let end = start + KEY_HEX_LEN;

        // encode only the key itself, not the DER header
        let der = self.0.to_pkcs1_der().unwrap();
        let key = &der.as_bytes()[start..end];
        key.encode_hex_upper()
    }
}

impl Debug for PrivateKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.encode_hex::<String>().fmt(f)
    }
}

// public key

const PUBLIC_PKCS1_DER_HEADER_LEN: usize = 9;

#[derive(Clone, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct PublicKey(RsaPublicKey);

impl PublicKey {
    pub fn to_der(&self) -> Vec<u8> {
        self.0.to_pkcs1_der().unwrap().as_bytes().to_vec()
    }

    pub fn verify(&self, msg: &[u8], sig: &[u8]) -> bool {
        self.0
            .verify(Pkcs1v15Sign::new::<Sha256>(), msg, sig)
            .is_ok()
    }
}

impl From<RsaPublicKey> for PublicKey {
    fn from(publ_key: RsaPublicKey) -> Self {
        Self(publ_key)
    }
}

impl ToHex for PublicKey {
    fn encode_hex<T: FromIterator<char>>(&self) -> T {
        let start = PUBLIC_PKCS1_DER_HEADER_LEN;
        let end = start + KEY_HEX_LEN;

        // encode only the key itself, not the DER header
        let der = self.0.to_pkcs1_der().unwrap();
        let key = &der.as_bytes()[start..end];
        key.encode_hex()
    }

    fn encode_hex_upper<T: FromIterator<char>>(&self) -> T {
        let start = PUBLIC_PKCS1_DER_HEADER_LEN;
        let end = start + KEY_HEX_LEN;

        // encode only the key itself, not the DER header
        let der = self.0.to_pkcs1_der().unwrap();
        let key = &der.as_bytes()[start..end];
        key.encode_hex_upper()
    }
}

impl Debug for PublicKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.encode_hex::<String>().fmt(f)
    }
}
