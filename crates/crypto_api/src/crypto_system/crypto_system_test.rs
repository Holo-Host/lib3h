//! Expose a test suite that can exercise CryptoSystem implementations.
//! You'll probably also need to write unit tests specific to your impl.

use crate::{Buffer, CryptoError, CryptoSystem};

struct FullSuite {
    crypto: Box<dyn CryptoSystem>,
}

impl FullSuite {
    pub fn new(crypto: Box<dyn CryptoSystem>) -> Self {
        FullSuite { crypto }
    }

    pub fn run(&self) {
        self.test_sec_buf();
        self.test_random();
        self.test_hash();
        self.test_pwhash();
        self.test_sign_keypair_sizes();
        self.test_sign_keypair_generation();
        self.test_sign();
    }

    fn test_sec_buf(&self) {
        let mut b1 = self.crypto.buf_new_secure(8);
        assert_eq!(8, b1.len());
        assert_eq!(
            "[0, 0, 0, 0, 0, 0, 0, 0]",
            &format!("{:?}", &b1.read_lock())
        );
        b1.write(0, &[42, 88, 132, 56, 12, 254, 212, 88]).unwrap();
        assert_eq!(
            "[42, 88, 132, 56, 12, 254, 212, 88]",
            &format!("{:?}", &b1.read_lock())
        );
        let b2 = b1.box_clone();
        b1.zero();
        assert_eq!(
            "[0, 0, 0, 0, 0, 0, 0, 0]",
            &format!("{:?}", &b1.read_lock())
        );
        assert_eq!(
            "[42, 88, 132, 56, 12, 254, 212, 88]",
            &format!("{:?}", &b2.read_lock())
        );
    }

    fn test_random(&self) {
        let mut a: Box<dyn Buffer> = Box::new(vec![0; 8]);
        let mut b: Box<dyn Buffer> = Box::new(vec![0; 8]);
        assert_eq!("[0, 0, 0, 0, 0, 0, 0, 0]", &format!("{:?}", a));
        self.crypto.randombytes_buf(&mut a).unwrap();
        self.crypto.randombytes_buf(&mut b).unwrap();
        assert_ne!("[0, 0, 0, 0, 0, 0, 0, 0]", &format!("{:?}", a));
        assert_ne!(&format!("{:?}", a), &format!("{:?}", b));
    }

    fn test_hash(&self) {
        let data: Box<dyn Buffer> = Box::new(vec![42, 1, 38, 2, 155, 212, 3, 11]);

        let mut hash256: Box<dyn Buffer> = Box::new(vec![0; self.crypto.hash_sha256_bytes()]);
        self.crypto.hash_sha256(&mut hash256, &data).unwrap();
        assert_eq!("[69, 32, 143, 143, 29, 27, 233, 62, 97, 209, 120, 159, 137, 193, 1, 213, 107, 128, 33, 170, 165, 131, 217, 170, 66, 192, 214, 190, 20, 179, 219, 177]", &format!("{:?}", hash256));

        let mut hash512: Box<dyn Buffer> = Box::new(vec![0; self.crypto.hash_sha512_bytes()]);
        self.crypto.hash_sha512(&mut hash512, &data).unwrap();
        assert_eq!("[105, 206, 48, 255, 80, 134, 192, 184, 108, 217, 124, 49, 193, 43, 2, 219, 148, 27, 91, 154, 89, 69, 229, 78, 13, 74, 51, 57, 52, 201, 186, 25, 109, 206, 155, 242, 249, 8, 179, 34, 106, 170, 160, 158, 11, 89, 85, 25, 22, 70, 70, 150, 84, 221, 184, 130, 245, 196, 101, 192, 160, 225, 160, 253]", &format!("{:?}", hash512));
    }

    fn test_pwhash(&self) {
        let mut pw: Box<dyn Buffer> = Box::new(vec![0; 16]);
        self.crypto.randombytes_buf(&mut pw).unwrap();
        let mut salt: Box<dyn Buffer> = Box::new(vec![0; self.crypto.pwhash_salt_bytes()]);
        self.crypto.randombytes_buf(&mut salt).unwrap();
        let mut hash1: Box<dyn Buffer> = Box::new(vec![0; self.crypto.pwhash_bytes()]);
        self.crypto.pwhash(&mut hash1, &pw, &salt).unwrap();
        let mut hash2: Box<dyn Buffer> = Box::new(vec![0; self.crypto.pwhash_bytes()]);
        self.crypto.pwhash(&mut hash2, &pw, &salt).unwrap();
        assert_eq!(&format!("{:?}", hash1), &format!("{:?}", hash2));
    }

    fn test_sign_keypair_sizes(&self) {
        let seed: Box<dyn Buffer> = Box::new(vec![0; self.crypto.sign_seed_bytes() + 1]);
        let mut pk: Box<dyn Buffer> = Box::new(vec![0; self.crypto.sign_public_key_bytes()]);
        let mut sk: Box<dyn Buffer> = Box::new(vec![0; self.crypto.sign_secret_key_bytes()]);
        assert_eq!(
            CryptoError::BadSeedSize,
            self.crypto
                .sign_seed_keypair(&seed, &mut pk, &mut sk)
                .unwrap_err()
        );

        let seed: Box<dyn Buffer> = Box::new(vec![0; self.crypto.sign_seed_bytes()]);
        let mut pk: Box<dyn Buffer> = Box::new(vec![0; self.crypto.sign_public_key_bytes() + 1]);
        let mut sk: Box<dyn Buffer> = Box::new(vec![0; self.crypto.sign_secret_key_bytes()]);
        assert_eq!(
            CryptoError::BadPublicKeySize,
            self.crypto
                .sign_seed_keypair(&seed, &mut pk, &mut sk)
                .unwrap_err()
        );
        assert_eq!(
            CryptoError::BadPublicKeySize,
            self.crypto.sign_keypair(&mut pk, &mut sk).unwrap_err()
        );

        let seed: Box<dyn Buffer> = Box::new(vec![0; self.crypto.sign_seed_bytes()]);
        let mut pk: Box<dyn Buffer> = Box::new(vec![0; self.crypto.sign_public_key_bytes()]);
        let mut sk: Box<dyn Buffer> = Box::new(vec![0; self.crypto.sign_secret_key_bytes() + 1]);
        assert_eq!(
            CryptoError::BadSecretKeySize,
            self.crypto
                .sign_seed_keypair(&seed, &mut pk, &mut sk)
                .unwrap_err()
        );
        assert_eq!(
            CryptoError::BadSecretKeySize,
            self.crypto.sign_keypair(&mut pk, &mut sk).unwrap_err()
        );
    }

    fn test_sign_keypair_generation(&self) {
        let mut seed: Box<dyn Buffer> = Box::new(vec![0; self.crypto.sign_seed_bytes()]);
        let mut pk1: Box<dyn Buffer> = Box::new(vec![0; self.crypto.sign_public_key_bytes()]);
        let mut sk1: Box<dyn Buffer> = Box::new(vec![0; self.crypto.sign_secret_key_bytes()]);
        let mut pk2: Box<dyn Buffer> = Box::new(vec![0; self.crypto.sign_public_key_bytes()]);
        let mut sk2: Box<dyn Buffer> = Box::new(vec![0; self.crypto.sign_secret_key_bytes()]);

        self.crypto
            .sign_seed_keypair(&seed, &mut pk1, &mut sk1)
            .unwrap();
        self.crypto
            .sign_seed_keypair(&seed, &mut pk2, &mut sk2)
            .unwrap();
        assert_eq!(&format!("{:?}", pk1), &format!("{:?}", pk2));
        assert_eq!(&format!("{:?}", sk1), &format!("{:?}", sk2));

        self.crypto.randombytes_buf(&mut seed).unwrap();
        self.crypto
            .sign_seed_keypair(&seed, &mut pk2, &mut sk2)
            .unwrap();
        assert_ne!(&format!("{:?}", pk1), &format!("{:?}", pk2));
        assert_ne!(&format!("{:?}", sk1), &format!("{:?}", sk2));

        self.crypto.sign_keypair(&mut pk1, &mut sk1).unwrap();
        assert_ne!(&format!("{:?}", pk1), &format!("{:?}", pk2));
        assert_ne!(&format!("{:?}", sk1), &format!("{:?}", sk2));
    }

    fn test_sign(&self) {
        let mut pk: Box<dyn Buffer> = Box::new(vec![0; self.crypto.sign_public_key_bytes()]);
        let mut sk: Box<dyn Buffer> = Box::new(vec![0; self.crypto.sign_secret_key_bytes()]);
        let mut msg: Box<dyn Buffer> = Box::new(vec![0; 64]);
        self.crypto.randombytes_buf(&mut msg).unwrap();

        self.crypto.sign_keypair(&mut pk, &mut sk).unwrap();

        let mut sig: Box<dyn Buffer> = Box::new(vec![0; self.crypto.sign_bytes()]);
        self.crypto.sign(&mut sig, &msg, &sk).unwrap();
        assert!(self.crypto.sign_verify(&sig, &msg, &pk).unwrap());

        self.crypto.randombytes_buf(&mut sig).unwrap();
        assert!(!self.crypto.sign_verify(&sig, &msg, &pk).unwrap());
    }
}

/// run a full suite of common CryptoSystem verification functions
pub fn full_suite(crypto: Box<dyn CryptoSystem>) {
    FullSuite::new(crypto).run();
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{CryptoResult, ProtectState};
    use sha2::Digest;
    use std::ops::{Deref, DerefMut};

    #[test]
    fn fake_should_pass_crypto_system_full_suite() {
        full_suite(Box::new(FakeCryptoSystem));
    }

    #[derive(Debug, Clone)]
    pub struct InsecureBuffer {
        b: Box<[u8]>,
        p: std::cell::RefCell<ProtectState>,
    }

    impl InsecureBuffer {
        pub fn new(size: usize) -> Self {
            InsecureBuffer {
                b: vec![0; size].into_boxed_slice(),
                p: std::cell::RefCell::new(ProtectState::NoAccess),
            }
        }
    }

    impl Deref for InsecureBuffer {
        type Target = [u8];

        fn deref(&self) -> &Self::Target {
            if *self.p.borrow() == ProtectState::NoAccess {
                panic!("Deref, but state is NoAccess");
            }
            &self.b
        }
    }

    impl DerefMut for InsecureBuffer {
        fn deref_mut(&mut self) -> &mut Self::Target {
            if *self.p.borrow() != ProtectState::ReadWrite {
                panic!("DerefMut, but state is not ReadWrite");
            }
            &mut self.b
        }
    }

    impl Buffer for InsecureBuffer {
        fn box_clone(&self) -> Box<dyn Buffer> {
            Box::new(self.clone())
        }
        fn as_buffer(&self) -> &dyn Buffer {
            &*self
        }
        fn as_buffer_mut(&mut self) -> &mut dyn Buffer {
            &mut *self
        }
        fn len(&self) -> usize {
            self.b.len()
        }
        fn is_empty(&self) -> bool {
            self.b.is_empty()
        }
        fn set_no_access(&self) {
            if *self.p.borrow() == ProtectState::NoAccess {
                panic!("already no access... bad logic");
            }
            *self.p.borrow_mut() = ProtectState::NoAccess;
        }
        fn set_readable(&self) {
            if *self.p.borrow() != ProtectState::NoAccess {
                panic!("not no access... bad logic");
            }
            *self.p.borrow_mut() = ProtectState::ReadOnly;
        }
        fn set_writable(&self) {
            if *self.p.borrow() != ProtectState::NoAccess {
                panic!("not no access... bad logic");
            }
            *self.p.borrow_mut() = ProtectState::ReadWrite;
        }
    }

    struct FakeCryptoSystem;

    impl CryptoSystem for FakeCryptoSystem {
        fn box_clone(&self) -> Box<dyn CryptoSystem> {
            Box::new(FakeCryptoSystem)
        }

        fn as_crypto_system(&self) -> &dyn CryptoSystem {
            &*self
        }

        fn buf_new_secure(&self, size: usize) -> Box<dyn Buffer> {
            Box::new(InsecureBuffer::new(size))
        }

        fn randombytes_buf(&self, buffer: &mut Box<dyn Buffer>) -> CryptoResult<()> {
            let mut buffer = buffer.write_lock();

            for i in 0..buffer.len() {
                buffer[i] = rand::random();
            }

            Ok(())
        }

        fn hash_sha256_bytes(&self) -> usize {
            32
        }
        fn hash_sha512_bytes(&self) -> usize {
            64
        }
        fn pwhash_salt_bytes(&self) -> usize {
            8
        }
        fn pwhash_bytes(&self) -> usize {
            16
        }

        fn hash_sha256(
            &self,
            hash: &mut Box<dyn Buffer>,
            data: &Box<dyn Buffer>,
        ) -> CryptoResult<()> {
            if hash.len() != self.hash_sha256_bytes() {
                return Err(CryptoError::BadHashSize);
            }

            let mut hasher = sha2::Sha256::new();
            hasher.input(data.read_lock().deref());
            hash.write(0, &hasher.result())?;
            Ok(())
        }

        fn hash_sha512(
            &self,
            hash: &mut Box<dyn Buffer>,
            data: &Box<dyn Buffer>,
        ) -> CryptoResult<()> {
            if hash.len() != self.hash_sha512_bytes() {
                return Err(CryptoError::BadHashSize);
            }

            let mut hasher = sha2::Sha512::new();
            hasher.input(data.read_lock().deref());
            hash.write(0, &hasher.result())?;
            Ok(())
        }

        fn pwhash(
            &self,
            hash: &mut Box<dyn Buffer>,
            password: &Box<dyn Buffer>,
            salt: &Box<dyn Buffer>,
        ) -> CryptoResult<()> {
            if hash.len() != self.pwhash_bytes() {
                return Err(CryptoError::BadHashSize);
            }

            if salt.len() != self.pwhash_salt_bytes() {
                return Err(CryptoError::BadSaltSize);
            }

            hash.write(0, &salt.read_lock())?;
            let plen = if password.len() > 8 {
                8
            } else {
                password.len()
            };
            hash.write(8, &password.read_lock()[0..plen])?;

            Ok(())
        }

        fn sign_seed_bytes(&self) -> usize {
            8
        }
        fn sign_public_key_bytes(&self) -> usize {
            32
        }
        fn sign_secret_key_bytes(&self) -> usize {
            8
        }
        fn sign_bytes(&self) -> usize {
            16
        }

        fn sign_seed_keypair(
            &self,
            seed: &Box<dyn Buffer>,
            public_key: &mut Box<dyn Buffer>,
            secret_key: &mut Box<dyn Buffer>,
        ) -> CryptoResult<()> {
            if seed.len() != self.sign_seed_bytes() {
                return Err(CryptoError::BadSeedSize);
            }

            if public_key.len() != self.sign_public_key_bytes() {
                return Err(CryptoError::BadPublicKeySize);
            }

            if secret_key.len() != self.sign_secret_key_bytes() {
                return Err(CryptoError::BadSecretKeySize);
            }

            secret_key.write(0, &seed.read_lock())?;

            public_key.zero();
            public_key.write(0, &seed.read_lock())?;

            Ok(())
        }

        fn sign_keypair(
            &self,
            public_key: &mut Box<dyn Buffer>,
            secret_key: &mut Box<dyn Buffer>,
        ) -> CryptoResult<()> {
            if public_key.len() != self.sign_public_key_bytes() {
                return Err(CryptoError::BadPublicKeySize);
            }

            if secret_key.len() != self.sign_secret_key_bytes() {
                return Err(CryptoError::BadSecretKeySize);
            }

            let mut seed: Box<dyn Buffer> = Box::new(vec![0; self.sign_seed_bytes()]);
            self.randombytes_buf(&mut seed)?;
            self.sign_seed_keypair(&seed, public_key, secret_key)?;

            Ok(())
        }

        fn sign(
            &self,
            signature: &mut Box<dyn Buffer>,
            message: &Box<dyn Buffer>,
            secret_key: &Box<dyn Buffer>,
        ) -> CryptoResult<()> {
            if signature.len() != self.sign_bytes() {
                return Err(CryptoError::BadSignatureSize);
            }

            if secret_key.len() != self.sign_secret_key_bytes() {
                return Err(CryptoError::BadSecretKeySize);
            }

            signature.write(0, &secret_key.read_lock())?;
            let mlen = if message.len() > 8 { 8 } else { message.len() };
            signature.write(8, &message.read_lock()[0..mlen])?;

            Ok(())
        }

        fn sign_verify(
            &self,
            signature: &Box<dyn Buffer>,
            message: &Box<dyn Buffer>,
            public_key: &Box<dyn Buffer>,
        ) -> CryptoResult<bool> {
            if signature.len() != self.sign_bytes() {
                return Err(CryptoError::BadSignatureSize);
            }

            if public_key.len() != self.sign_public_key_bytes() {
                return Err(CryptoError::BadPublicKeySize);
            }

            let signature = signature.read_lock();
            let mlen = if message.len() > 8 { 8 } else { message.len() };

            Ok(&signature[0..8] == &public_key.read_lock()[0..8]
                && &signature[8..mlen + 8] == &message.read_lock()[0..mlen])
        }
    }
}
