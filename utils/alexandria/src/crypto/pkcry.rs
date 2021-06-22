use crate::{
    crypto::CipherText,
    error::{Error, Result},
    wire::Encoder,
};
use sodiumoxide::crypto::box_::{self, Nonce, PublicKey, SecretKey};

/// A wrapper around an NaCl public key
pub struct PubKey {
    inner: PublicKey,
}

impl PubKey {
    pub(crate) fn seal<T>(&self, data: &T, auth: &SecKey) -> Result<CipherText>
    where
        T: Encoder<T>,
    {
        let non = box_::gen_nonce();
        let enc = data.encode()?;
        let data = box_::seal(&enc, &non, &self.inner, &auth.inner);
        let nonce = non.0.iter().cloned().collect();
        Ok(CipherText { nonce, data })
    }
}

/// A wrapper around an NaCl secret key
pub struct SecKey {
    inner: SecretKey,
}

impl SecKey {
    pub fn open<T>(&self, data: &CipherText, auth: &PubKey) -> Result<T>
    where
        T: Encoder<T>,
    {
        let CipherText {
            ref nonce,
            ref data,
        } = data;

        let nonce =
            Nonce::from_slice(&nonce.as_slice()).ok_or(Error::internal("Failed to read nonce!"))?;

        let clear = box_::open(data.as_slice(), &nonce, &auth.inner, &self.inner)
            .map_err(|_| Error::internal("Failed to decrypt data"))?;

        Ok(T::decode(&clear)?)
    }
}
