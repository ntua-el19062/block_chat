use crate::{
    blockchain::{block::Block, transaction::Transaction, Blockchain},
    crypto::PublicKey,
    peer::PeersCatalog,
};
use serde::{Deserialize, Serialize};
use std::{
    io::{self, Write as _},
    net::{IpAddr, SocketAddr, TcpListener, TcpStream, ToSocketAddrs},
    num::NonZeroU32,
    thread,
    time::Duration,
};

#[derive(Clone, Deserialize, Serialize)]
struct PeerInfo {
    publ_key: PublicKey,
    ip: IpAddr,
    net_port: u16,
    bs_port: u16,
}

#[derive(Deserialize, Serialize)]
enum BootstrapMessage {
    JoinRequest {
        publ_key: PublicKey,
        net_port: u16,
        bs_port: u16,
    },

    JoinResponse {
        peers_info: Vec<PeerInfo>,
        blockchain: Blockchain,
    },
}

pub fn bootstrap_network(
    total_peers: u16,
    cents_per_peer: u32,
    bootstrap_peer_addr: impl ToSocketAddrs,
    bootstrap_port: u16,
    network_port: u16,
    publ_key: PublicKey,
) -> (TcpListener, PeersCatalog, Blockchain) {
    assert!(total_peers > 1, "The network size cannot be less than 2");
    assert!(cents_per_peer > 0, "The cents per peer cannot be 0");
    assert!(bootstrap_port > 0, "The bootstrap port cannot be 0");
    let bootstrap_peer_addr = bootstrap_peer_addr
        .to_socket_addrs()
        .expect("Failed to resolve bootstrap address")
        .next()
        .unwrap();

    let (bs_listener, bs_port) = bind_listener(bootstrap_port).unwrap();
    let (net_listener, net_port) = bind_listener(network_port).unwrap();

    send_join_request(bootstrap_peer_addr, publ_key.clone(), net_port, bs_port);

    let (peers_info, blockchain) = match discover_peers(bs_listener, total_peers, publ_key.clone())
    {
        (peers_info, Some(blockchain)) => (peers_info, blockchain),
        (peers_info, None) => {
            let blockchain = init_blockchain(&peers_info, NonZeroU32::new(cents_per_peer).unwrap());
            send_join_responses(peers_info.clone(), blockchain.clone());
            (peers_info, blockchain)
        }
    };

    let mut catalog = PeersCatalog::new();
    for peer in peers_info {
        catalog
            .insert((peer.publ_key, (peer.ip, peer.net_port).into()))
            .unwrap();
    }

    (net_listener, catalog, blockchain)
}

fn bind_listener(port: u16) -> Result<(TcpListener, u16), io::Error> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port))?;
    let addr = listener.local_addr().unwrap();

    log::debug!("Bootstrap: Listener bound to {}", addr);

    Ok((listener, addr.port()))
}

fn send_join_request(
    bs_peer_addr: impl ToSocketAddrs,
    publ_key: PublicKey,
    net_port: u16,
    bs_port: u16,
) {
    let bs_peer_addr = bs_peer_addr
        .to_socket_addrs()
        .expect("Failed to resolve the bootstrap peer's address")
        .next()
        .unwrap();

    let req = BootstrapMessage::JoinRequest {
        publ_key,
        net_port,
        bs_port,
    };

    let req_bytes = serde_json::to_vec(&req).expect("Failed to serialize join request");

    thread::spawn(move || loop {
        let mut stream = match TcpStream::connect(bs_peer_addr) {
            Ok(stream) => stream,
            Err(e) => {
                log::warn!("Bootstrap: Failed to connect to bootstrap node: {}", e);
                thread::sleep(Duration::from_secs(1));
                continue;
            }
        };

        if let Err(e) = stream.write_all(&req_bytes) {
            log::warn!("Bootstrap: Failed to send join request: {}", e);
            thread::sleep(Duration::from_secs(1));
            continue;
        }

        log::debug!("Bootstrap: Join request successfully sent to the bootstrap node");

        break;
    });
}

fn discover_peers(
    listener: TcpListener,
    total_peers: u16,
    publ_key: PublicKey,
) -> (Vec<PeerInfo>, Option<Blockchain>) {
    let mut discovered_peers = vec![];
    let mut added_self = false;

    loop {
        let (mut stream, addr) = match listener.accept() {
            Ok((stream, addr)) => (stream, addr),
            Err(e) => {
                log::warn!("Bootstrap: Failed to accept incoming connection: {}", e);
                continue;
            }
        };

        let mut de = serde_json::Deserializer::from_reader(&mut stream);
        let message = match BootstrapMessage::deserialize(&mut de) {
            Ok(message) => message,
            Err(e) => {
                log::warn!("Bootstrap: Failed to deserialize message: {}", e);
                continue;
            }
        };

        let peer_info = match message {
            BootstrapMessage::JoinRequest {
                publ_key,
                net_port,
                bs_port,
            } => PeerInfo {
                publ_key,
                ip: addr.ip(),
                net_port,
                bs_port,
            },

            BootstrapMessage::JoinResponse {
                peers_info,
                blockchain,
            } => {
                return (peers_info, Some(blockchain));
            }
        };

        if !added_self && peer_info.publ_key == publ_key {
            discovered_peers.push(peer_info);
            let last = discovered_peers.len() - 1;
            discovered_peers.swap(0, last); // move self to the front
            added_self = true;
        } else {
            discovered_peers.push(peer_info);
        }

        if discovered_peers.len() as u16 >= total_peers {
            return (discovered_peers, None);
        }
    }
}

fn init_blockchain(peer_info: &[PeerInfo], amnt_per_peer: NonZeroU32) -> Blockchain {
    let gen_tsxs = peer_info
        .iter()
        .map(|p| Transaction::new_genesis(p.publ_key.clone(), amnt_per_peer))
        .collect::<Vec<_>>();
    let gen_blk = Block::new_genesis(gen_tsxs);
    Blockchain::new(gen_blk)
}

fn send_join_responses(peers_info: Vec<PeerInfo>, blockchain: Blockchain) {
    let bs_addrs: Vec<SocketAddr> = peers_info
        .iter()
        .skip(1)
        .map(|peer| (peer.ip, peer.bs_port).into())
        .collect::<Vec<_>>();

    let res = BootstrapMessage::JoinResponse {
        peers_info,
        blockchain,
    };

    let res_bytes = serde_json::to_vec(&res).expect("Failed to serialize join response");

    let mut ok = 0;
    for addr in bs_addrs {
        let mut stream = match TcpStream::connect(addr) {
            Ok(stream) => stream,
            Err(e) => {
                log::warn!("Bootstrap: Failed to connect to peer: {}", e);
                continue;
            }
        };

        if let Err(e) = stream.write_all(&res_bytes) {
            log::warn!("Bootstrap: Failed to send join response: {}", e);
        } else {
            ok += 1;
        }
    }

    log::trace!(
        "Bootstrap: Join responses successfully sent to {} peers",
        ok
    );
}
