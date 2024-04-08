use block_chat::{
    cli::{Args, Command},
    history::History,
    protocol::Broadcast,
};
use clap::Parser as _;
use env_logger::Env;
use std::{
    env,
    io::{self, Read as _, Write as _},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs as _},
};

// environment variable to set the logging level
const LOGGIN_LEVEL_ENV: &str = "BLOCK_CHAT_CLIENT_LOGGING_LEVEL";
const DEFAULT_LOGGING_LEVEL: &str = "warn";

// environment variables to set the daemon address (defaults to `localhost:27737`)
const DAEMON_SOCKET_ENV: &str = "BLOCK_CHAT_DAEMON_SOCKET";
const DAEMON_PORT_ENV: &str = "BLOCK_CHAT_DAEMON_PORT";
const DEFAULT_DAEMON_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
const DEFAULT_DAEMON_PORT: u16 = 27737;

fn main() -> io::Result<()> {
    // display message if arguments are incorrect (clap does this automatically)
    let command = Args::parse().cmd;

    // initialize logger and daemon address
    init_logger();
    let daemon_addr = init_daemon_addr();

    log::debug!("Daemon address: {}", daemon_addr);

    // send the command and wait for the response
    let response = send_command_receive_response(command.clone(), daemon_addr)?;

    if matches!(command, Command::H) {
        // when the command is 'history' the response has to be deserialized
        let response: History =
            serde_json::from_slice(&response).expect("Failed to deserialize response");

        println!("{}", response);
    } else {
        // when the command is not 'history' the response is just a string
        println!("{}", String::from_utf8(response).unwrap());
    }

    Ok(())
}

fn init_logger() {
    let env = Env::new().filter_or(LOGGIN_LEVEL_ENV, DEFAULT_LOGGING_LEVEL);
    env_logger::init_from_env(env);
}

fn init_daemon_addr() -> SocketAddr {
    if let Ok(addr) = env::var(DAEMON_SOCKET_ENV) {
        return addr
            .to_socket_addrs()
            .unwrap_or_else(|_| {
                panic!(
                    "Environment variable `{}` could not be parsed as a valid socket address",
                    DAEMON_SOCKET_ENV
                )
            })
            .next()
            .unwrap();
    }

    if let Ok(port) = env::var(DAEMON_PORT_ENV) {
        let port = port.parse().unwrap_or_else(|_| {
            panic!(
                "Environment variable `{}` could not be parsed as a valid port number",
                DAEMON_PORT_ENV
            )
        });

        return SocketAddr::new(DEFAULT_DAEMON_IP, port);
    };

    SocketAddr::new(DEFAULT_DAEMON_IP, DEFAULT_DAEMON_PORT)
}

fn send_command_receive_response(cmd: Command, addr: SocketAddr) -> io::Result<Vec<u8>> {
    let mut stream = TcpStream::connect(addr)?;

    let cmd_bytes =
        serde_json::to_vec(&Broadcast::Command(cmd)).expect("Failed to serialize command");

    stream.write_all(&cmd_bytes)?;

    let mut buf = vec![];
    stream.read_to_end(&mut buf)?;

    Ok(buf)
}
