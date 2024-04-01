use block_chat::{
    crypto::PrivateKey,
    protocol::{Protocol, ProtocolConfig},
};
use env_logger::Env;
use rsa::RsaPrivateKey;
use std::{
    env,
    net::{SocketAddr, ToSocketAddrs},
};

const LOGGIN_LEVEL_ENV: &str = "BLOCK_CHAT_DAEMON_LOGGING_LEVEL";
const FALLBACK_LOGGING_LEVEL: &str = "warn";

const BOOTSTRAP_PEER_SOCKET_ENV: &str = "BLOCK_CHAT_BOOTSTRAP_PEER_SOCKET";

const BOOTTSRAP_PORT_ENV: &str = "BLOCK_CHAT_BOOTSTRAP_PORT";
const DEFAULT_BOOTSTRAP_PORT: u16 = 27736;

const NETWORK_PORT_ENV: &str = "BLOCK_CHAT_NETWORK_PORT";
const DEFAULT_NETWORK_PORT: u16 = 27737;

const NETWORK_SIZE_ENV: &str = "BLOCK_CHAT_NETWORK_SIZE";
const DEFAULT_NETWORK_SIZE: u16 = 5;

const INIT_COINS_PER_PEER: u32 = 1000;

const RSA_BITS: usize = 2048;

fn main() {
    init_logger();
    let bootstrap_peer_addr = init_bootstrap_peer_addr();
    let bootstrap_port = init_bootstrap_port();
    let network_port = init_network_port();
    let network_size = init_network_size();

    log::debug!("Bootstrap peer address: {}", bootstrap_peer_addr);
    log::debug!("Bootstrap port: {}", bootstrap_port);
    log::debug!("Network port: {}", network_port);
    log::debug!("Network size: {}", network_size);

    let priv_key = PrivateKey::from(RsaPrivateKey::new(&mut rand::thread_rng(), RSA_BITS).unwrap());

    let config = ProtocolConfig {
        total_peers: network_size,
        init_coins_per_peer: INIT_COINS_PER_PEER,
        bootstrap_peer_addr,
        bootstrap_port,
        network_port,
    };

    let mut protocol = Protocol::new(priv_key);

    protocol.run(config);
}

fn init_logger() {
    let env = Env::new().filter_or(LOGGIN_LEVEL_ENV, FALLBACK_LOGGING_LEVEL);
    env_logger::init_from_env(env);
}

fn init_bootstrap_peer_addr() -> SocketAddr {
    env::var(BOOTSTRAP_PEER_SOCKET_ENV)
        .unwrap_or_else(|_| {
            panic!(
                "Environment variable `{}` must be set to a valid socket address",
                BOOTSTRAP_PEER_SOCKET_ENV
            )
        })
        .to_socket_addrs()
        .unwrap_or_else(|_| {
            panic!(
                "Environment variable `{}` could not be parsed as a valid socket address",
                BOOTSTRAP_PEER_SOCKET_ENV
            )
        })
        .next()
        .unwrap()
}

fn init_bootstrap_port() -> u16 {
    env::var(BOOTTSRAP_PORT_ENV).map_or(DEFAULT_BOOTSTRAP_PORT, |port| {
        port.parse().unwrap_or_else(|_| {
            panic!(
                "Environment variable `{}` could not be parsed as a valid port number",
                BOOTTSRAP_PORT_ENV
            )
        })
    })
}

fn init_network_port() -> u16 {
    env::var(NETWORK_PORT_ENV).map_or(DEFAULT_NETWORK_PORT, |port| {
        port.parse().unwrap_or_else(|_| {
            panic!(
                "Environment variable `{}` could not be parsed as a valid port number",
                NETWORK_PORT_ENV
            )
        })
    })
}

fn init_network_size() -> u16 {
    env::var(NETWORK_SIZE_ENV).map_or(DEFAULT_NETWORK_SIZE, |size| {
        size.parse().unwrap_or_else(|_| {
            panic!(
                "Environment variable `{}` could not be parsed as a valid number",
                NETWORK_SIZE_ENV
            )
        })
    })
}
