use wavs::config::Config;
use wavs_types::Credential;

pub fn mock_config() -> Config {
    Config {
        submission_mnemonic: Some(Credential::new(
            "test test test test test test test test test test test junk".to_string(),
        )),
        ..wavs::config::Config::default()
    }
}
