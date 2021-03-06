// Copyright 2020 WeDPR Lab Project Authors. Licensed under Apache-2.0.

//! Secp256k1 signature functions.

#[macro_use]
extern crate wedpr_l_macros;
#[macro_use]
extern crate lazy_static;

extern crate secp256k1;
use secp256k1::{
    recovery::{RecoverableSignature, RecoveryId},
    All, Message, Secp256k1, SecretKey, VerifyOnly,
};
use wedpr_l_utils::{error::WedprError, traits::Signature};

lazy_static! {
    // Shared secp256k1 instance initialized for verification function only.
    static ref SECP256K1_VERIFY: Secp256k1<VerifyOnly> = Secp256k1::verification_only();
    // Shared secp256k1 instance initialized for all functions.
    static ref SECP256K1_ALL: Secp256k1<All> = Secp256k1::new();
}

/// Implements FISCO-BCOS-compatible Secp256k1 as a Signature instance.
#[derive(Default, Debug, Clone, Copy)]
pub struct WedprSecp256k1Recover {}

const FISCO_BCOS_SIGNATURE_DATA_LENGTH: usize = 65;
const FISCO_BCOS_SIGNATURE_END_INDEX: usize =
    FISCO_BCOS_SIGNATURE_DATA_LENGTH - 1;

impl Signature for WedprSecp256k1Recover {
    fn sign<T: ?Sized + AsRef<[u8]>>(
        &self,
        private_key: &T,
        msg_hash: &T,
    ) -> Result<Vec<u8>, WedprError> {
        let secret_key = match SecretKey::from_slice(&private_key.as_ref()) {
            Ok(v) => v,
            Err(_) => {
                wedpr_println!("Parsing private key failed");
                return Err(WedprError::FormatError);
            },
        };
        // Message hash length for Secp256k1 signature should be 32 bytes.
        let msg_hash_obj = match Message::from_slice(&msg_hash.as_ref()) {
            Ok(v) => v,
            Err(_) => {
                wedpr_println!("Parsing message hash failed");
                return Err(WedprError::FormatError);
            },
        };
        let signature_obj =
            SECP256K1_ALL.sign_recoverable(&msg_hash_obj, &secret_key);
        let (recid, signature_bytes) = &signature_obj.serialize_compact();
        // Append recovery id to the end of signature bytes.
        let mut signature_output = signature_bytes.to_vec();
        signature_output.push(recid.to_i32() as u8);
        // The signature data contains two parts:
        // sig\[0..64\): signature for the message hash.
        // sig\[64\]: recovery id.
        Ok(signature_output)
    }

    fn verify<T: ?Sized + AsRef<[u8]>>(
        &self,
        public_key: &T,
        msg_hash: &T,
        signature: &T,
    ) -> bool {
        // Message hash length for Secp256k1 signature should be 32 bytes.
        let recover_public_key = match self.recover_public_key(msg_hash, signature) {
            Ok(v) => v,
            Err(_) => return false,
        };
        if recover_public_key.ne(&public_key.as_ref().to_vec()) {
            wedpr_println!("Matching signature public key failed");
            return false;
        }
        return true;
    }

    fn generate_keypair(&self) -> (Vec<u8>, Vec<u8>) {
        let mut rng = rand::thread_rng();
        loop {
            // "rand" feature of secp256k1 need to be enabled for this.
            let (secret_key, public_key) =
                SECP256K1_ALL.generate_keypair(&mut rng);
            // Drop weak secret key.
            if secret_key[0] > 15 {
                return (
                    public_key.serialize_uncompressed().to_vec(),
                    secret_key.as_ref().to_vec(),
                );
            }
        }
    }
}

impl WedprSecp256k1Recover {
    pub fn recover_public_key<T: ?Sized + AsRef<[u8]>>(
        self,
        msg_hash: &T,
        signature: &T,
    ) -> Result<Vec<u8>, WedprError> {
        // Message hash length for Secp256k1 signature should be 32 bytes.
        let msg_hash_obj = match Message::from_slice(&msg_hash.as_ref()) {
            Ok(v) => v,
            Err(_) => {
                wedpr_println!("Parsing message hash failed");
                return Err(WedprError::DecodeError);
            },
        };
        if signature.as_ref().len() != FISCO_BCOS_SIGNATURE_DATA_LENGTH {
            wedpr_println!("Signature length is not 65");
            return Err(WedprError::DecodeError);
        };
        let rec_id = match RecoveryId::from_i32(
            signature.as_ref()[FISCO_BCOS_SIGNATURE_END_INDEX] as i32,
        ) {
            Ok(v) => v,
            Err(_) => {
                wedpr_println!("Parsing RecoveryId failed");
                return Err(WedprError::DecodeError);
            },
        };

        // The last byte is recovery id, we only need to get the first 64 bytes
        // for signature data.
        let signature_byte =
            &signature.as_ref()[0..FISCO_BCOS_SIGNATURE_END_INDEX];

        let get_sign_final =
            match RecoverableSignature::from_compact(signature_byte, rec_id) {
                Ok(v) => v,
                Err(_) => {
                    wedpr_println!("Signature from_compact failed");
                    return Err(WedprError::FormatError);
                },
            };
        let recovered_public_key =
            match SECP256K1_VERIFY.recover(&msg_hash_obj, &get_sign_final) {
                Ok(v) => v,
                Err(_) => {
                    wedpr_println!("Signature recover failed");
                    return Err(WedprError::FormatError);
                },
            };
        return Ok(recovered_public_key.serialize_uncompressed().to_vec());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wedpr_l_utils::constant::tests::BASE64_ENCODED_TEST_MESSAGE;

    #[test]
    fn test_secp256k1_recover() {
        let secp256k1 = WedprSecp256k1Recover::default();
        let (public_key, secret_key) = secp256k1.generate_keypair();

        // The message hash (NOT the original message) is required for
        // generating a valid signature.
        let msg_hash = BASE64_ENCODED_TEST_MESSAGE;

        let signature =
            secp256k1.sign(&secret_key, &msg_hash.to_vec()).unwrap();
        assert_eq!(
            true,
            secp256k1.verify(
                &public_key.to_vec(),
                &msg_hash.to_vec(),
                &signature
            )
        );
    }
}
