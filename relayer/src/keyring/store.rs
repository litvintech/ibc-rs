
use std::collections::BTreeMap;
use k256::{
    ecdsa::{signature::Signer, signature::Verifier, Signature, SigningKey, VerifyKey},
    EncodedPoint, SecretKey,
};
use bitcoin_wallet::account::MasterAccount;
use bitcoin_wallet::mnemonic::Mnemonic;
use bitcoin::{
    network::constants::Network,
    util::bip32::{DerivationPath, ExtendedPrivKey, ExtendedPubKey},
    PrivateKey,
};
use hdpath::StandardHDPath;
use bitcoin::secp256k1::{All, Message, Secp256k1};
use std::convert::TryFrom;
use crate::keyring::errors::{Error, Kind};

pub type Address = Vec<u8>;

pub enum KeyRing {
    MemoryKeyStore { store: BTreeMap<Address, KeyEntry> }
}

pub enum StoreBackend {
    Memory
}

pub trait KeyRingOperations: Sized {
    fn init(backend: StoreBackend) -> KeyRing;
    fn add_from_mnemonic(&mut self, mnemonic_words: &str) -> Result<Address, Error>;
    fn get(&self, address: Vec<u8>) -> Result<&KeyEntry, Error>;
    fn insert(&mut self, addr: Vec<u8>, key: KeyEntry) -> Option<KeyEntry>;
    fn sign(&self, signer: Vec<u8>, msg: Vec<u8>) -> Vec<u8>;
}

/// Key entry stores the Private Key and Public Key as well the address
#[derive(Clone, Debug)]
pub struct KeyEntry {
    /// Public key
    pub public_key: ExtendedPubKey,

    /// Private key
    pub private_key: ExtendedPrivKey,
}

impl KeyRingOperations for KeyRing {

    /// Initialize a in memory key entry store
    fn init(backend: StoreBackend) -> KeyRing {
        match backend {
            StoreBackend::Memory => {
                let store: BTreeMap<Address, KeyEntry> = BTreeMap::new();
                KeyRing::MemoryKeyStore { store }
            }
        }
    }

    /// Add a key entry in the store using a mnemonic.
    fn add_from_mnemonic(&mut self, mnemonic_words: &str) -> Result<Address, Error> {

        // Generate seed from mnemonic
        let mnemonic = Mnemonic::from_str(mnemonic_words).map_err(|e| Kind::InvalidMnemonic.context(e))?;
        let seed = mnemonic.to_seed(Some(""));

        // Get Private Key from seed and standard derivation path
        let hd_path = StandardHDPath::try_from("m/44'/118'/0'/0/0").unwrap();
        let private_key = ExtendedPrivKey::new_master(Network::Bitcoin, &seed.0)
            .and_then(|k| k.derive_priv(&Secp256k1::new(), &DerivationPath::from(hd_path))).map_err(|e| Kind::PrivateKey.context(e))?;

        // Get Public Key from Private Key
        let public_key = ExtendedPubKey::from_private(&Secp256k1::new(), &private_key);

        // Get address from the Public Key
        let address = get_address(public_key);

        let key = KeyEntry {
            public_key,
            private_key
        };

        self.insert(address.clone(), key);

        Ok(address)
    }

    /// Return a key entry from a key name
    fn get(&self, address: Vec<u8>) -> Result<&KeyEntry, Error> {
        match &self {
            KeyRing::MemoryKeyStore { store: s } => {
                if !s.contains_key(&address) {
                    return Err(Kind::InvalidKey.into());
                }
                else {
                    let key = s.get(&address);
                    match key {
                        Some(k) => Ok(k),
                        None => Err(Kind::InvalidKey.into())
                    }
                }
            }
        }
    }

    /// Insert an entry in the key store
    fn insert(&mut self, addr: Vec<u8>, key: KeyEntry) -> Option<KeyEntry> {
        match self {
            KeyRing::MemoryKeyStore { store: s} => {
                let ke = s.insert(addr, key);
                ke
            }
        }
    }

    /// Sign a message
    fn sign(&self, signer: Vec<u8>, msg: Vec<u8>) -> Vec<u8> {
        let key = self.get(signer).unwrap();
        let private_key_bytes = key.private_key.private_key.to_bytes();
        let signing_key = SigningKey::new(private_key_bytes.as_slice()).unwrap();
        let signature: Signature = signing_key.sign(&msg);
        signature.as_ref().to_vec()
    }
}

/// Return an address from a Public Key
fn get_address(pk: ExtendedPubKey) -> Vec<u8> {
    use crypto::digest::Digest;
    use crypto::ripemd160::Ripemd160;
    use crypto::sha2::Sha256;

    let mut sha256 = Sha256::new();
    sha256.input(pk.public_key.to_bytes().as_slice());
    let mut bytes = vec![0; sha256.output_bytes()];
    sha256.result(&mut bytes);
    let mut hash = Ripemd160::new();
    hash.input(bytes.as_slice());
    let mut acct = vec![0; hash.output_bytes()];
    hash.result(&mut acct);
    acct.to_vec()
}