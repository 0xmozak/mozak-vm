use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub rpc: RPC,
}

#[derive(Deserialize)]
pub struct RPC {
    pub host: String,
    pub port: u16,
}

pub fn generate_default_and_save(_config_path: &str) {
    unimplemented!();
}
