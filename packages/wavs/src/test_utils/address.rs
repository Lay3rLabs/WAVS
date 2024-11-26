use bip39::Mnemonic;
use layer_climb::prelude::*;
use rand::prelude::*;

pub fn rand_address() -> Address {
    let mut rng = rand::thread_rng();

    let entropy: [u8; 32] = rng.gen();
    let mnemonic = Mnemonic::from_entropy(&entropy).unwrap();

    let signer = KeySigner::new_mnemonic_iter(mnemonic.words(), None).unwrap();

    let public_key = signer.key.public_key();
    let public_key_bytes = public_key.to_bytes();
    let public_key = PublicKey::from_raw_secp256k1(&public_key_bytes).unwrap();

    Address::new_cosmos_pub_key(&public_key, "layer").unwrap()
}
