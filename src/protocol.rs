use crate::{
    account::{Account, AccountsCatalog},
    blockchain::{
        block::{Block, BlockValidator, BLOCK_CAPACITY},
        transaction::{Transaction, TransactionValidator},
        Blockchain,
    },
    bootstrap::bootstrap_network,
    cli::Command,
    crypto::PrivateKey,
    history::History,
    peer::{Peer, PeersCatalog},
};
use non_empty_string::NonEmptyString;
use rand::{RngCore as _, SeedableRng as _};
use rand_chacha::ChaCha12Rng;
use serde::{Deserialize, Serialize};
use std::{
    cell::Cell,
    io::Write as _,
    net::{TcpListener, TcpStream, ToSocketAddrs},
    num::NonZeroU32,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

// unsafe static muts, used only for benchmarking
static mut TSX_START: Option<Instant> = None;
static mut BLK_START: Option<Instant> = None;
static mut TSX_TIMES: Vec<Duration> = vec![];
static mut BLK_TIMES: Vec<Duration> = vec![];

// the user sees coins as floating point numbers
// but the program uses integers to avoid floating point errors
// so whenever the user wants to create a transaction
// a conversion is needed
pub const CENTS_PER_COIN: u32 = 100;
pub const TRANSFER_FEE_PERCENTAGE: u32 = 3;
pub const MESSAGE_FEE_PER_CHARACTER_CENTS: u32 = CENTS_PER_COIN;
pub const MINIMUM_TRANSFER_FEE_CENTS: u32 = 1;

// multiplex Transactions, Blocks and Commands on the same TCP socket
#[derive(Deserialize, Serialize)]
pub enum Broadcast {
    Transaction(Transaction),
    Block(Block),
    Command(Command),
}

pub struct ProtocolConfig<A: ToSocketAddrs> {
    pub total_peers: u16,         // how many peers are in the network
    pub init_coins_per_peer: u32, // how many coins each peer starts with
    pub bootstrap_peer_addr: A,   // the address of the bootstrap peer
    pub bootstrap_port: u16,      // the port to be used for the bootstrap process
    pub network_port: u16,        // the port to be used for the network
}

struct ProtocolState<'a> {
    id: u32,
    peers: &'static PeersCatalog,
    soft_accounts: AccountsCatalog<'a>,
    hard_accounts: AccountsCatalog<'a>,
    pending_transactions: Vec<Transaction>,
    blockchain: Blockchain,

    // memoization of `proof_of_stake()`
    next_validator_id: Cell<Option<u32>>,

    // for transaction and block broadcasting
    tx: Sender<Broadcast>,
}

pub struct Protocol<'a> {
    priv_key: PrivateKey,
    state: Option<ProtocolState<'a>>,
}

impl<'a> Protocol<'a> {
    pub fn new(priv_key: PrivateKey) -> Self {
        Self {
            priv_key,
            state: None,
        }
    }

    pub fn run(&mut self, cfg: ProtocolConfig<impl ToSocketAddrs>) {
        fn spawn_listener_thread(listener: TcpListener, tx: Sender<(Broadcast, TcpStream)>) {
            debug_assert!(listener.local_addr().is_ok());

            thread::spawn(move || {
                for conn in listener.incoming() {
                    let stream = match conn {
                        Ok(stream) => stream,
                        Err(e) => {
                            log::warn!("Listener: Failed to establish connection: {}", e);
                            continue;
                        }
                    };

                    let mut de = serde_json::Deserializer::from_reader(stream.try_clone().unwrap());
                    let broadcast = match Broadcast::deserialize(&mut de) {
                        Ok(broadcast) => broadcast,
                        Err(e) => {
                            log::warn!("Listener: Failed to deserialize stream data: {}", e);
                            continue;
                        }
                    };

                    log::trace!(
                        "Listener: Received {} from {}",
                        match &broadcast {
                            Broadcast::Transaction(_) => "transaction",
                            Broadcast::Block(_) => "block",
                            Broadcast::Command(_) => "command",
                        },
                        stream.peer_addr().unwrap()
                    );

                    tx.send((broadcast, stream)).unwrap();
                }
            });
        }

        fn spawn_broadcast_thread(rx: Receiver<Broadcast>, id: u32, peers: &'static PeersCatalog) {
            thread::spawn(move || {
                for broadcast in rx {
                    let broadcast_bytes =
                        serde_json::to_vec(&broadcast).expect("Failed to serialize transaction");

                    for addr in peers
                        .iter()
                        .filter(|peer| peer.id() != id)
                        .map(|peer| peer.sock_addr())
                    {
                        let mut stream = match TcpStream::connect(addr) {
                            Ok(stream) => stream,
                            Err(e) => {
                                log::warn!("Broadcast: Failed to connect to peer: {}", e);
                                continue;
                            }
                        };

                        if let Err(e) = stream.write_all(&broadcast_bytes) {
                            log::warn!("Broadcast: Failed to broadcast: {}", e);
                        }
                    }
                }
            });
        }

        // bootstrapping
        let (network_listener, peers, blockchain) = bootstrap_network(
            cfg.total_peers,
            cfg.init_coins_per_peer * CENTS_PER_COIN,
            cfg.bootstrap_peer_addr,
            cfg.bootstrap_port,
            cfg.network_port,
            self.priv_key.to_publ_key(),
        );

        debug_assert!(blockchain.len() == 1);

        log::debug!(
            "Protocol: Discovered {} peers: {:#?}",
            peers.len(),
            peers.iter().map(|p| p.sock_addr()).collect::<Vec<_>>()
        );

        // make the list of peers static so it can be used in threads
        // this creates a memory leak anytime a Protocol object is dropped
        // however this is not a problem since the Protocol
        // object is only dropped if the program exits
        let peers = peers.leak();

        // create an account for each peer and process the genesis transactions
        let mut hard_accounts = AccountsCatalog::new(peers);
        for tsx in blockchain.last_block().tsxs() {
            hard_accounts.process_transaction(tsx).unwrap();
        }

        // find the local peer id
        let id = peers
            .get_by_publ_key(&self.priv_key.to_publ_key())
            .unwrap()
            .id();

        // spawn the thread that will handle broadcasting transactions and blocks
        // the reason this is done is mainly to ensure that two consecutive blocks
        // are ALWAYS sent in the correct order
        // transactions can be technically sent in any order, however it's still desirable
        // to have them in the correct order
        // broadcasting is done on a separate thread in order to avoid blocking the main thread
        let (tx, rx): (Sender<Broadcast>, _) = mpsc::channel();
        spawn_broadcast_thread(rx, id, peers);

        self.state = Some(ProtocolState {
            id,
            peers,
            soft_accounts: hard_accounts.clone(),
            hard_accounts,
            pending_transactions: vec![],
            blockchain,
            next_validator_id: Cell::new(None),
            tx,
        });

        // spawn the thread that will listen for incoming transactions and blocks
        // this needs to be done on a separate thread
        // otherwise the main thread would constantly block
        let (tx, rx): (Sender<(Broadcast, TcpStream)>, _) = mpsc::channel();
        spawn_listener_thread(network_listener, tx);

        // sloppy code only used for benchmarking
        unsafe {
            BLK_START.replace(Instant::now());
            TSX_START.replace(Instant::now());
        }

        // main loop
        for event in rx {
            match event {
                (Broadcast::Transaction(tsx), _) => self.handle_transaction(tsx, None, false),
                (Broadcast::Block(blk), _) => self.handle_block(blk, false),
                (Broadcast::Command(command), stream) => self.handle_command(command, stream),
            }
        }
    }

    // main reason for these methods method is to avoid constantly '.as_ref/mut().unwrap()'ing
    fn state(&self) -> &ProtocolState<'a> {
        self.state.as_ref().expect("Protocol not running")
    }

    fn state_mut(&mut self) -> &mut ProtocolState<'a> {
        self.state.as_mut().expect("Protocol not running")
    }

    fn local_peer(&self) -> &Peer {
        self.state()
            .peers
            .get_by_id(self.state().id)
            .expect("Local peer not found")
    }

    fn network_peer(&self, id: u32) -> Option<&Peer> {
        self.state().peers.get_by_id(id)
    }

    fn local_soft_account(&self) -> &Account {
        self.state()
            .soft_accounts
            .get_by_id(self.state().id)
            .expect("Local account not found")
    }

    fn handle_command(&mut self, command: Command, mut stream: TcpStream) {
        // t command
        fn new_transfer(
            protocol: &Protocol,
            recp_id: u32,
            amnt: NonZeroU32,
            stream: &mut TcpStream,
        ) -> Option<Transaction> {
            let sndr = protocol.local_peer();
            let sndr_acc = protocol.local_soft_account();

            if recp_id == sndr.id() {
                if let Err(e) = stream.write_all("You cannot send coins to yourself".as_bytes()) {
                    log::warn!("Failed to respond to `t` command: {}", e);
                } else {
                    log::trace!("Successfully responded to `t` command");
                }
                return None;
            }

            let recp = match protocol.network_peer(recp_id) {
                Some(peer) => peer,
                None => {
                    if let Err(e) = stream.write_all("Recipient not found".as_bytes()) {
                        log::warn!("Failed to respond to `t` command: {}", e);
                    } else {
                        log::trace!("Successfully responded to `t` command");
                    }
                    return None;
                }
            };

            // coins to cents conversion
            let amnt_cents = amnt
                .checked_mul(CENTS_PER_COIN.try_into().unwrap())
                .unwrap();

            if sndr_acc.held_cents() < Transaction::calculate_transfer_total_cost(amnt_cents) {
                if let Err(e) = stream.write_all("Not enough coins".as_bytes()) {
                    log::warn!("Failed to respond to `t` command: {}", e);
                } else {
                    log::trace!("Successfully responded to `t` command");
                }
                return None;
            }

            Some(Transaction::new_transfer(
                sndr.publ_key().clone(),
                recp.publ_key().clone(),
                amnt_cents,
                sndr_acc.nonce_pool().next(),
                &protocol.priv_key,
            ))
        }

        // m command
        fn new_message(
            protocol: &Protocol,
            recp_id: u32,
            message: String,
            stream: &mut TcpStream,
        ) -> Option<Transaction> {
            let sndr = protocol.local_peer();
            let sndr_acc = protocol.local_soft_account();

            if recp_id == sndr.id() {
                if let Err(e) = stream.write_all("You cannot message yourself".as_bytes()) {
                    log::warn!("Failed to respond to `m` command: {}", e);
                } else {
                    log::trace!("Successfully responded to `m` command");
                }
                return None;
            }

            let recp = match protocol.network_peer(recp_id) {
                Some(peer) => peer,
                None => {
                    if let Err(e) = stream.write_all("Recipient not found".as_bytes()) {
                        log::warn!("Failed to respond to `m` command: {}", e);
                    } else {
                        log::trace!("Successfully responded to `m` command");
                    }
                    return None;
                }
            };

            let message = match NonEmptyString::new(message) {
                Ok(message) => message,
                Err(_) => {
                    if let Err(e) = stream.write_all("Message cannot be empty".as_bytes()) {
                        log::warn!("Failed to respond to `m` command: {}", e);
                    } else {
                        log::trace!("Successfully responded to `m` command");
                    }
                    return None;
                }
            };

            if sndr_acc.held_cents() < Transaction::calculate_message_total_cost(&message) {
                if let Err(e) = stream.write_all("Not enough coins".as_bytes()) {
                    log::warn!("Failed to respond to `m` command: {}", e);
                } else {
                    log::trace!("Successfully responded to `m` command");
                }
                return None;
            }

            Some(Transaction::new_message(
                sndr.publ_key().clone(),
                recp.publ_key().clone(),
                message,
                sndr_acc.nonce_pool().next(),
                &protocol.priv_key,
            ))
        }

        // s command
        fn new_stake(
            protocol: &Protocol,
            amnt: NonZeroU32,
            stream: &mut TcpStream,
        ) -> Option<Transaction> {
            let sndr = protocol.local_peer();
            let sndr_acc = protocol.local_soft_account();

            // coins to cents conversion
            let amnt_cents = amnt
                .checked_mul(CENTS_PER_COIN.try_into().unwrap())
                .unwrap();

            if sndr_acc.held_cents() < Transaction::calculate_stake_total_cost(amnt_cents) {
                if let Err(e) = stream.write_all("Not enough coins".as_bytes()) {
                    log::warn!("Failed to respond to `stake` command: {}", e);
                } else {
                    log::trace!("Successfully responded to `stake` command");
                }
                return None;
            }

            Some(Transaction::new_stake(
                sndr.publ_key().clone(),
                amnt_cents,
                sndr_acc.nonce_pool().next(),
                &protocol.priv_key,
            ))
        }

        // b command
        fn send_balance(account: &Account, stream: &mut TcpStream) {
            let reply = format!(
                "Balance: {} held, {} staked",
                account.held_cents() as f64 / CENTS_PER_COIN as f64,
                account.staked_cents() as f64 / CENTS_PER_COIN as f64
            );

            if let Err(e) = stream.write_all(reply.as_bytes()) {
                log::warn!("Failed to respond to `balance` command: {}", e);
            } else {
                log::trace!("Successfully responded to `balance` command");
            }
        }

        // v command
        fn send_last_block(blockchain: &Blockchain, stream: &mut TcpStream) {
            let reply = format!("Last block: {:#?}", blockchain.last_block());

            if let Err(e) = stream.write_all(reply.as_bytes()) {
                log::warn!("Failed to respond to `view` command: {}", e);
            } else {
                log::trace!("Successfully responded to `view` command");
            }
        }

        // h command
        fn send_history(history: History, stream: &mut TcpStream) {
            let history_bytes = serde_json::to_vec(&history).expect("Failed to serialize history");

            if let Err(e) = stream.write_all(&history_bytes) {
                log::warn!("Failed to respond to `history` command: {}", e);
            } else {
                log::trace!("Successfully responded to `history` command");
            }
        }

        use Command::*;
        match command {
            T { rcp_id, amt } => {
                if let Some(tsx) = new_transfer(self, rcp_id, amt, &mut stream) {
                    self.handle_transaction(tsx, Some(stream), true);
                }
            }

            M { rcp_id, msg } => {
                if let Some(tsx) = new_message(self, rcp_id, msg.join(" "), &mut stream) {
                    self.handle_transaction(tsx, Some(stream), true);
                }
            }

            S { amt } => {
                if let Some(tsx) = new_stake(self, amt, &mut stream) {
                    self.handle_transaction(tsx, Some(stream), true);
                }
            }

            B => send_balance(self.local_soft_account(), &mut stream),
            V => send_last_block(&self.state().blockchain, &mut stream),
            H => send_history(History::global_history(), &mut stream),

            // the client is not programmed to send I commands
            I => unreachable!(),

            // used only by the helper to determine which file to read from during benchmarking
            Id => stream
                .write_all(self.local_peer().id().to_string().as_bytes())
                .unwrap(),

            // used only for benchmarking
            // calculate the average transaction time
            // and the average block time
            Time => {
                let tsx_times = unsafe { TSX_TIMES.clone() };
                let blk_times = unsafe { BLK_TIMES.clone() };

                let tsx_avg = tsx_times.iter().sum::<Duration>() / tsx_times.len() as u32;
                let blk_avg = blk_times.iter().sum::<Duration>() / blk_times.len() as u32;

                let reply = format!(
                    "Average transaction time 1: {} ms\nAverage block time 1: {} ms\n",
                    tsx_avg.as_secs_f64() * 1000.0,
                    blk_avg.as_secs_f64() * 1000.0,
                );

                stream.write_all(reply.as_bytes()).unwrap()
            }

            // used only for benchmarking
            Stats => {
                let reply = History::global_stats();

                stream.write_all(reply.as_bytes()).unwrap();
            }
        }
    }

    fn handle_transaction(&mut self, tsx: Transaction, stream: Option<TcpStream>, is_local: bool) {
        if is_local {
            History::log_local_transaction(&tsx, self.state().peers);

            // these should never panic for locally created transactions
            // why would we create an invalid transaction?
            #[cfg(debug_assertions)] // == only execute in debug mode
            if let Err(e) = TransactionValidator::validate_structure(&tsx) {
                panic!("Debug assertion failed: {}", e);
            }

            #[cfg(debug_assertions)]
            if let Err(e) =
                TransactionValidator::validate_semantics(&tsx, &self.state().soft_accounts)
            {
                panic!("Debug assertion failed: {}", e);
            }
        } else {
            History::log_network_transaction(&tsx, self.state().peers);

            // validate the structure of the transaction (ignore context)
            if let Err(e) = TransactionValidator::validate_structure(&tsx) {
                History::log_invalid_transaction(&tsx, self.state().peers);
                log::warn!("Received invalid transaction:\n{}\n{:#?}", e, tsx);
                return;
            }

            // validate the semantics of the transaction (the soft_accounts is the context)
            if let Err(e) =
                TransactionValidator::validate_semantics(&tsx, &self.state().soft_accounts)
            {
                History::log_invalid_transaction(&tsx, self.state().peers);
                log::warn!("Received invalid transaction:\n{}\n{:#?}", e, tsx);
                return;
            }
        }

        // this should not panic (due to the previous 2 calls)
        self.state_mut()
            .soft_accounts
            .process_transaction(&tsx)
            .unwrap();

        self.state_mut().pending_transactions.push(tsx.clone());

        if let Some(mut stream) = stream {
            if let Err(e) = stream.write_all("Transaction successful".as_bytes()) {
                log::warn!("Failed to send success to client: {}", e);
            } else {
                log::trace!("Successfully sent success to client");
            }
        }

        if is_local {
            self.broadcast_transaction(tsx);
        }

        // * tsx time end
        unsafe {
            TSX_TIMES.push(TSX_START.take().unwrap().elapsed());
            TSX_START.replace(Instant::now());
        }

        self.try_mint_block();
    }

    fn handle_block(&mut self, blk: Block, is_local: bool) {
        if is_local {
            History::log_local_block(&blk, self.state().peers);

            // these should never panic for locally created blocks
            // why would we create an invalid block?
            #[cfg(debug_assertions)] // == only execute in debug mode
            if let Err(e) = BlockValidator::validate_structure(&blk) {
                panic!("Debug assertion failed: {}", e);
            }

            #[cfg(debug_assertions)]
            if let Err(e) = BlockValidator::validate_semantics(
                &blk,
                self.proof_of_stake(),
                (&self.state().hard_accounts, &self.state().blockchain),
            ) {
                panic!("Debug assertion failed: {}", e);
            }
        } else {
            History::log_network_block(&blk, self.state().peers);

            // validate the structure of the block (ignore context)
            if let Err(e) = BlockValidator::validate_structure(&blk) {
                History::log_invalid_block(&blk, self.state().peers);
                log::warn!("Received invalid block:\n{}\n{:#?}", e, blk);
                return;
            }

            // validate the semantics of the block
            // (the hard_accounts and blockchain are the context)
            if let Err(e) = BlockValidator::validate_semantics(
                &blk,
                self.proof_of_stake(),
                (&self.state().hard_accounts, &self.state().blockchain),
            ) {
                History::log_invalid_block(&blk, self.state().peers);
                log::warn!("Received invalid block:\n{}\n{:#?}", e, blk);
                return;
            }
        }

        // this should not panic (due to the previous 2 calls)
        self.state_mut().hard_accounts.process_block(&blk).unwrap();

        self.state_mut().blockchain.add_block(blk.clone()); // add to blockchain
        self.state_mut().next_validator_id.set(None); // reset memoized validator

        if is_local {
            self.broadcast_block(blk.clone());
        }

        let mut new_soft_accounts = self.state().hard_accounts.clone();
        let peers = self.state().peers;

        // discard all transactions pending in the block and reprocess the rest
        self.state_mut().pending_transactions.retain(|p_tsx| {
            // TODO: this could probably be sped up by using a HashSet, but it's not that important
            blk.tsxs().iter().all(|b_tsx| p_tsx.hash() != b_tsx.hash())
                && if new_soft_accounts.process_transaction(p_tsx).is_err() {
                    History::log_invalid_transaction(p_tsx, peers);
                    false // discard now-invalid transactions
                } else {
                    true // keep the rest
                }
        });

        // update soft accounts
        self.state_mut().soft_accounts = new_soft_accounts;

        // * blk time end
        unsafe {
            BLK_TIMES.push(BLK_START.take().unwrap().elapsed());
            BLK_START.replace(Instant::now());
        }

        self.try_mint_block();
    }

    fn try_mint_block(&mut self) {
        // if the block is not full or if the node is not the validator return
        if self.state().pending_transactions.len() < BLOCK_CAPACITY
            || self.state().id != self.proof_of_stake()
        {
            return;
        }

        let transactions: [Transaction; BLOCK_CAPACITY] = self
            .state_mut()
            .pending_transactions
            .drain(..BLOCK_CAPACITY)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        let block = Block::new(
            transactions,
            self.priv_key.to_publ_key(),
            *self.state().blockchain.last_block().hash(),
        );

        self.handle_block(block, true);
    }

    fn broadcast_transaction(&self, tsx: Transaction) {
        self.state().tx.send(Broadcast::Transaction(tsx)).unwrap();
    }

    fn broadcast_block(&self, blk: Block) {
        self.state().tx.send(Broadcast::Block(blk)).unwrap();
    }

    fn proof_of_stake(&self) -> u32 {
        fn calculate_tickets(staked_cents: u32) -> u32 {
            staked_cents
        }

        // memoization
        if let Some(id) = self.state().next_validator_id.get() {
            return id;
        }

        // the total amount of tickets in the lottery
        let stake_sum = self
            .state()
            .hard_accounts
            .iter()
            .map(|acc| calculate_tickets(acc.staked_cents()))
            .sum::<u32>();

        // if no one has staked, the validator is selected randomly
        // and every peer has the same chance of being chosen
        let tickets = if stake_sum == 0 {
            self.state().hard_accounts.len() as u32
        } else {
            stake_sum
        };

        let seed = self.state().blockchain.last_block().hash();
        let mut rng = ChaCha12Rng::from_seed(*seed);

        // select a random ticket
        let winning_ticket = rng.next_u32() % tickets;

        let winner_id = if stake_sum == 0 {
            self.state().hard_accounts[winning_ticket as usize].id()
        } else {
            let mut acc = 0;
            self.state()
                .hard_accounts
                .iter()
                // when the accumulator exceeds the winning ticket, the winner is found
                .find(|account| {
                    acc += calculate_tickets(account.staked_cents());
                    acc > winning_ticket
                })
                .unwrap()
                .id()
        };

        // memoization
        self.state().next_validator_id.set(Some(winner_id));
        History::log_new_validator(self.state().id, winner_id, &self.state().blockchain);

        winner_id
    }
}
