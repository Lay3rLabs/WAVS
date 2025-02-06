use alloy::signers::local::{coins_bip39::English, MnemonicBuilder};
use bip39::Mnemonic;
use layer_climb::prelude::*;
use rand::prelude::*;

pub fn rand_address_eth() -> alloy::primitives::Address {
    let mut rng = rand::rng();

    let entropy: [u8; 32] = rng.random();
    let mnemonic = Mnemonic::from_entropy(&entropy).unwrap();

    let signer = MnemonicBuilder::<English>::default()
        .phrase(mnemonic.words().collect::<Vec<&str>>().join(" "))
        .build()
        .unwrap();

    signer.address()
}

pub fn rand_event_eth() -> [u8; 32] {
    rand::rng().random()
}

pub fn rand_event_cosmos() -> String {
    const_hex::encode(rand_event_eth())
}

pub fn rand_address_layer() -> Address {
    let mut rng = rand::rng();

    let entropy: [u8; 32] = rng.random();
    let mnemonic = Mnemonic::from_entropy(&entropy).unwrap();

    let signer = KeySigner::new_mnemonic_iter(mnemonic.words(), None).unwrap();

    let public_key = signer.key.public_key();
    let public_key_bytes = public_key.to_bytes();
    let public_key = PublicKey::from_raw_secp256k1(&public_key_bytes).unwrap();

    Address::new_cosmos_pub_key(&public_key, "layer").unwrap()
}
