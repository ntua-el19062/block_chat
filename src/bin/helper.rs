use std::{
    env,
    error::Error,
    fs,
    io::{self, BufRead as _, Read, Write as _},
    net::{SocketAddr, TcpStream, ToSocketAddrs},
    num::NonZeroU32,
    thread,
    time::Duration,
};

// This binary is only used for benchmarking the application.
// It reads a file with a list of commands and sends them to the daemon.

const DAEMON_SOCKET_ENV: &str = "DAEMON_SOCKET";
const FIXED_STAKING_ENV: &str = "FIXED_STAKING";
const INPUT_FOLDER_ENV: &str = "INPUT_FOLDER";

fn main() -> Result<(), Box<dyn Error>> {
    let daemon_addr = env::var(DAEMON_SOCKET_ENV)
        .unwrap_or_else(|_| panic!("{} not set", DAEMON_SOCKET_ENV))
        .to_socket_addrs()?
        .next()
        .unwrap();

    let fixed_staking: NonZeroU32 = env::var(FIXED_STAKING_ENV)
        .unwrap_or_else(|_| panic!("{} not set", FIXED_STAKING_ENV))
        .parse::<u32>()
        .unwrap()
        .try_into()
        .expect("The fixed staking must be positive");

    let input_folder =
        env::var(INPUT_FOLDER_ENV).unwrap_or_else(|_| panic!("{} not set", INPUT_FOLDER_ENV));

    let id = loop {
        let id_cmd = block_chat::cli::Command::Id;
        match send_cmd(id_cmd, daemon_addr) {
            Ok(res) => break String::from_utf8(res)?,
            Err(_) => thread::sleep(Duration::from_secs(1)),
        }
    };

    let filename = format!("{}/trans{}.txt", input_folder, id);
    let file = fs::File::open(&filename)?;

    let stake_cmd = block_chat::cli::Command::S { amt: fixed_staking };
    send_cmd(stake_cmd, daemon_addr)?;

    println!("Helper starting; reading from {}", filename);

    let mut count = 0;
    for line in io::BufReader::new(file).lines() {
        let line = line?;
        let words = line.split_whitespace().collect::<Vec<_>>();
        let rcp_id = words[0].strip_prefix("id").unwrap().parse::<u32>()?;
        let msg = words
            .iter()
            .skip(1)
            .map(|s| s.to_string())
            .collect::<Vec<_>>();

        let cmd = block_chat::cli::Command::M { rcp_id, msg };
        send_cmd(cmd, daemon_addr)?;

        count += 1;
    }

    println!("Helper finished; sent {} commands", count);

    Ok(())
}

fn send_cmd(cmd: block_chat::cli::Command, addr: SocketAddr) -> Result<Vec<u8>, Box<dyn Error>> {
    let req = block_chat::protocol::Broadcast::Command(cmd);
    let req_bytes = serde_json::to_vec(&req)?;
    let mut res_bytes = vec![];

    let mut stream = TcpStream::connect(addr)?;
    stream.write_all(&req_bytes)?;
    stream.read_to_end(&mut res_bytes)?;

    Ok(res_bytes)
}
