//! Test-only helpers: owner keys and OAU1 signing for the authority, enrollment and
//! ownership tests. The ML-DSA side signs with RustCrypto `ml-dsa` (dev-dependency,
//! the independent oracle); devices verify with libcrux.

use crate::authority::{encode_oau1, signed_message, AuthorityKeys, AUTH_DOMAIN, MLDSA44_PK_LEN, SUITE_HYBRID};
use ml_dsa::{Keypair, MlDsa44, SigningKey, B32};

pub const NET: [u8; 8] = *b"OASISnet";

/// An owner (or authority) key pair: Ed25519 + ML-DSA-44 test seeds.
pub struct TestOwner {
    pub ed_seed: [u8; 32],
    pub ml_seed: [u8; 32],
    pub ed_pub: [u8; 32],
    pub ml_pub: [u8; MLDSA44_PK_LEN],
}

impl TestOwner {
    pub fn new(ed_seed: [u8; 32], ml_seed: [u8; 32]) -> Self {
        let kp = ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::new(ed_seed));
        let mut ed_pub = [0u8; 32];
        ed_pub.copy_from_slice(kp.pk.as_ref());
        let mut ml_pub = [0u8; MLDSA44_PK_LEN];
        ml_pub.copy_from_slice(&SigningKey::<MlDsa44>::from_seed(&B32::from(ml_seed)).verifying_key().encode());
        TestOwner { ed_seed, ml_seed, ed_pub, ml_pub }
    }

    pub fn keys(&self) -> AuthorityKeys<'_> {
        AuthorityKeys { ed25519: self.ed_pub, mldsa44: &self.ml_pub }
    }

    /// Sign `content` as an OAU1 message (`suite` = Ed25519 or hybrid) on `net`.
    pub fn sign(&self, kind: u8, suite: u8, net: &[u8; 8], content: &[u8]) -> Vec<u8> {
        let msg = signed_message(kind, suite, net, content);
        let kp = ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::new(self.ed_seed));
        let mut ed = [0u8; 64];
        ed.copy_from_slice(kp.sk.sign(&msg, None).as_ref());
        let ml = if suite == SUITE_HYBRID {
            SigningKey::<MlDsa44>::from_seed(&B32::from(self.ml_seed))
                .expanded_key()
                .sign_deterministic(&msg, AUTH_DOMAIN)
                .unwrap()
                .encode()
                .to_vec()
        } else {
            Vec::new()
        };
        encode_oau1(kind, suite, net, content, &ed, &ml)
    }
}

/// Owner #1 (the E/F operator seeds), the new owner and an attacker.
pub fn o1() -> TestOwner {
    TestOwner::new([0x0E; 32], [0x0F; 32])
}
pub fn o2() -> TestOwner {
    TestOwner::new([0x2E; 32], [0x2F; 32])
}
pub fn o3() -> TestOwner {
    TestOwner::new([0x3E; 32], [0x3F; 32])
}
