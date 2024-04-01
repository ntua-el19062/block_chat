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
    iter,
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs as _},
};

const LOGGIN_LEVEL_ENV: &str = "BLOCK_CHAT_CLIENT_LOGGING_LEVEL";
const FALLBACK_LOGGING_LEVEL: &str = "warn";

const DAEMON_SOCKET_ENV: &str = "BLOCK_CHAT_DAEMON_SOCKET";
const DAEMON_PORT_ENV: &str = "BLOCK_CHAT_DAEMON_PORT";
const DEFAULT_DAEMON_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
const DEFAULT_DAEMON_PORT: u16 = 27737;

fn main() -> io::Result<()> {
    // display message if arguments are incorrect
    let command = Args::parse().cmd;

    init_logger();
    let daemon_addr = init_daemon_addr();

    log::debug!("Daemon address: {}", daemon_addr);

    if matches!(command, Command::I) {
        interactive_mode(daemon_addr);
        return Ok(());
    }

    let response = send_command_receive_response(command.clone(), daemon_addr)?;

    if matches!(command, Command::H) {
        let response: History =
            serde_json::from_slice(&response).expect("Failed to deserialize response");

        println!("{}", response);
    } else {
        println!("{}", String::from_utf8(response).unwrap());
    }

    Ok(())
}

fn init_logger() {
    let env = Env::new().filter_or(LOGGIN_LEVEL_ENV, FALLBACK_LOGGING_LEVEL);
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

fn interactive_mode(daemon_addr: SocketAddr) {
    loop {
        print!("Enter a command (or 'exit'): ");
        io::stdout().flush().unwrap();

        let mut line = String::new();
        io::stdin().read_line(&mut line).unwrap();

        if line.trim() == "exit" {
            println!(); // add missing newline
            break;
        }

        let args = iter::once("client")
            .chain(line.split_whitespace())
            .collect::<Vec<&str>>();

        let command = match Args::try_parse_from(args) {
            Ok(args) => args.cmd,
            Err(e) => {
                eprintln!("{}", e);
                continue;
            }
        };

        let response = match send_command_receive_response(command.clone(), daemon_addr) {
            Ok(response) => response,
            Err(e) => {
                eprintln!("Failed to send command: {}", e);
                continue;
            }
        };

        if matches!(command, Command::H) {
            let response: History =
                serde_json::from_slice(&response).expect("Failed to deserialize response");

            println!("{}", response);
        } else {
            println!("{}", String::from_utf8(response).unwrap());
        }
    }
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
