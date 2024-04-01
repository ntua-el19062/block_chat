use std::{
    env,
    error::Error,
    fs,
    io::{self, BufRead as _, Read, Write as _},
    net::{TcpStream, ToSocketAddrs},
    thread,
    time::Duration,
};

fn main() -> Result<(), Box<dyn Error>> {
    let network_size = env::var("BLOCK_CHAT_NETWORK_SIZE")
        .unwrap_or(u32::MAX.to_string())
        .parse::<u32>()?;

    let daemon_addr = env::var("BLOCK_CHAT_DAEMON_SOCKET")
        .expect("BLOCK_CHAT_DAEMON_ADDR not set")
        .to_socket_addrs()?
        .next()
        .unwrap();

    let req = serde_json::to_vec(&block_chat::protocol::Broadcast::Command(
        block_chat::cli::Command::Id,
    ))?;

    let mut res = vec![];

    let mut stream;
    loop {
        stream = match TcpStream::connect(daemon_addr) {
            Ok(stream) => stream,
            Err(_) => {
                thread::sleep(Duration::from_secs(1));
                continue;
            }
        };

        if stream.write_all(&req).is_err() {
            continue;
        }

        if stream.read_to_end(&mut res).is_ok() {
            break;
        }
    }

    let id = String::from_utf8(res)?;
    let filename = format!("input/trans{}.txt", id);
    let file = fs::File::open(&filename)?;

    println!("Helper starting; reading from {}", filename);

    let mut count = 0;
    for line in io::BufReader::new(file).lines() {
        let line = line?;
        let words = line.split_whitespace().collect::<Vec<_>>();
        let rcp_id = words[0].strip_prefix("id").unwrap().parse::<u32>()? % network_size;
        let msg = words
            .iter()
            .skip(1)
            .map(|s| s.to_string())
            .collect::<Vec<_>>();

        let cmd =
            block_chat::protocol::Broadcast::Command(block_chat::cli::Command::M { rcp_id, msg });

        let bytes = serde_json::to_vec(&cmd)?;

        // add a random delay (up to 0.15sec) before sending the message
        thread::sleep(Duration::from_millis(rand::random::<u64>() % 150));

        stream = TcpStream::connect(daemon_addr)?;
        stream.write_all(&bytes)?;

        let mut res = vec![];
        stream.read_to_end(&mut res)?;

        count += 1;
    }

    println!("Helper finished; sent {} commands", count);

    Ok(())
}
