use clap::Parser;
use serde::{de, Deserialize, Deserializer, Serialize};
use std::{fmt, path::PathBuf};

/// This struct is used for both CliArgs and Environment variables
/// Every Cli Arg can be overridden by an environment variable
/// following the pattern of AGGREGATOR_{UPPERCASE_ARG_NAME}
/// where "AGGREGATOR" is configured in the CliArgs::ENV_VAR_PREFIX constant
#[derive(Debug, Parser, Serialize, Deserialize, Default)]
#[command(version, about, long_about = None)]
#[serde(default)]
pub struct CliArgs {
    /// The home directory of the application, where the aggregator.toml configuration file is stored
    /// if not provided, a series of default directories will be tried
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub home: Option<PathBuf>,

    /// The path to an optional dotenv file to try and load
    /// if not set, will be the current working directory's .env
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dotenv: Option<PathBuf>,

    /// The port to bind the server to.
    /// See example config file for more info
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u32>,

    /// Log level in the format of comma-separated tracing directives.
    /// See example config file for more info
    #[arg(long, value_delimiter = ',')]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(deserialize_with = "deserialize_vec_string")]
    pub log_level: Vec<String>,

    /// The directory to store all internal data files
    /// See example config file for more info
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<PathBuf>,

    /// The host to bind the server to
    /// See example config file for more info
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,

    /// The allowed cors origins
    /// See example config file for more info
    #[arg(long, value_delimiter = ',')]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(deserialize_with = "deserialize_vec_string")]
    pub cors_allowed_origins: Vec<String>,

    /// Websocket ethereum endpoint
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ws_endpoint: Option<String>,

    /// Http ethereum endpoint
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_endpoint: Option<String>,

    /// Ethereum mnemonic
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mnemonic: Option<String>,

    /// Hd index
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hd_index: Option<u32>,

    /// Number of tasks before submitting transaction
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tasks_quorum: Option<u32>,
}

impl CliArgs {
    pub const ENV_VAR_PREFIX: &'static str = "AGGREGATOR";

    pub fn env_var(name: &str) -> Option<String> {
        std::env::var(format!("{}_{name}", Self::ENV_VAR_PREFIX)).ok()
    }
}

fn deserialize_vec_string<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringOrVec;

    impl<'de> de::Visitor<'de> for StringOrVec {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a comma-separated string or a sequence of strings")
        }

        fn visit_str<E>(self, value: &str) -> Result<Vec<String>, E>
        where
            E: de::Error,
        {
            Ok(value.split(',').map(|s| s.trim().to_string()).collect())
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Vec<String>, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            let mut vec = Vec::new();
            while let Some(elem) = seq.next_element()? {
                vec.push(elem);
            }
            Ok(vec)
        }
    }

    deserializer.deserialize_any(StringOrVec)
}
