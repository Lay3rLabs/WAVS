use alloy_signer_local::{coins_bip39::English, MnemonicBuilder};
use bip39::Mnemonic;
use layer_climb::prelude::*;
use rand::prelude::*;
use wavs_types::ByteArray;

pub fn rand_address_evm() -> alloy_primitives::Address {
    let mut rng = rand::rng();

    let entropy: [u8; 32] = rng.random();
    let mnemonic = Mnemonic::from_entropy(&entropy).unwrap();

    let signer = MnemonicBuilder::<English>::default()
        .phrase(mnemonic.words().collect::<Vec<&str>>().join(" "))
        .build()
        .unwrap();

    signer.address()
}

pub fn rand_event_evm() -> ByteArray<32> {
    ByteArray::new(rand::rng().random())
}

pub fn rand_event_cosmos() -> String {
    const_hex::encode(rand_event_evm().as_slice())
}

pub fn rand_address_cosmos() -> CosmosAddr {
    let mut rng = rand::rng();

    let entropy: [u8; 32] = rng.random();
    let mnemonic = Mnemonic::from_entropy(&entropy).unwrap();

    let signer = KeySigner::new_mnemonic_iter(mnemonic.words(), None).unwrap();

    let public_key = signer.key.public_key();
    let public_key_bytes = public_key.to_bytes();
    let public_key = PublicKey::from_raw_secp256k1(&public_key_bytes).unwrap();

    CosmosAddr::new_pub_key(&public_key, "layer").unwrap()
}
