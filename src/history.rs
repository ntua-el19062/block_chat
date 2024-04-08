use std::{
    collections::BTreeMap,
    fmt::{self, Display, Formatter},
};

use serde::{Deserialize, Serialize};

use crate::{
    blockchain::{
        block::Block,
        transaction::{Transaction, TransactionPayload},
        Blockchain,
    },
    peer::PeersCatalog,
    protocol::CENTS_PER_COIN,
};

static mut GLOBAL_HISTORY: History = History(vec![]);

// A struct to keep track of the history of the blockchain
// used only for debugging purposes.

/*
    The following are considered noteworthy events:
    - a transaction (transfer, message, stake) is created locally
    - a block is created locally
    - a transaction (transfer, message, stake) is received from the network
    - a block is received from the network
    - a transaction is found to be invalid
    - a block is found to be invalid
    - a new validator is elected

    The id is a unique identifier for the event as follows:
    - for transactions, it is the combination of the sender's id and nonce
    - for blocks, it is the first 8 characters of the hash of the block
    - for new validator events, it is the index of the last block in the blockchain

    Each transaction event also includes the source and destination account ids,
    as well as the amount or message of the transaction

    Each block event includes the validator's id and the ids of the transactions in the block
*/

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum EventKind {
    // Local Transfer
    LT { amount: f64 },
    // Local Message
    LM { message: String },
    // Local Stake
    LS { amount: f64 },
    // Local Block
    LB { tids: Vec<String> },
    // Network Transfer
    NT { amount: f64 },
    // Network Message
    NM { message: String },
    // Network Stake
    NS { amount: f64 },
    // Network Block
    NB { tids: Vec<String> },
    // Invalid Transaction
    IT,
    // Invalid Block
    IB,
    // New Validator Elected
    NV { vid: u32 },
}

#[derive(Clone, Serialize, Deserialize)]
struct Event {
    id: String,
    src: u32,
    dst: Option<u32>,
    #[serde(rename = "type")]
    #[serde(flatten)]
    kind: EventKind,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct History(Vec<Event>);

impl History {
    pub fn global_stats() -> String {
        let history = unsafe { GLOBAL_HISTORY.clone() };

        // number of transactions sent by each peer
        // create a BTreeMap with id as index and total transactions as value
        let mut total_tsx = 0;
        let mut txs_sent = BTreeMap::new();
        for event in &history.0 {
            match &event.kind {
                EventKind::LT { .. }
                | EventKind::LM { .. }
                | EventKind::LS { .. }
                | EventKind::NT { .. }
                | EventKind::NM { .. }
                | EventKind::NS { .. } => {
                    total_tsx += 1;
                    *txs_sent.entry(event.src).or_insert(0) += 1;
                }
                _ => (),
            }
        }

        // number of blocks validated by each peer
        // create a BTreeMap with id as index and total blocks as value
        let mut toal_blk = 0;
        let mut blk_validated = BTreeMap::new();
        for event in &history.0 {
            match &event.kind {
                EventKind::LB { .. } | EventKind::NB { .. } => {
                    toal_blk += 1;
                    *blk_validated.entry(event.src).or_insert(0) += 1;
                }
                _ => (),
            }
        }

        // number of invalid transactions sent by each peer
        let mut total_itsx = 0;
        let mut itsx_sent = BTreeMap::new();
        for event in &history.0 {
            if matches!(event.kind, EventKind::IT) {
                total_itsx += 1;
                *itsx_sent.entry(event.src).or_insert(0) += 1;
            }
        }

        // number of invalid blocks validated by each peer
        let mut total_iblk = 0;
        let mut iblk_validated = BTreeMap::new();
        for event in &history.0 {
            if matches!(event.kind, EventKind::IB) {
                total_iblk += 1;
                *iblk_validated.entry(event.src).or_insert(0) += 1;
            }
        }

        /*
            Peer 0 made 10 transactions and validated 5 blocks
            [Peer 0 made 3 invalid transactions and validated 2 invalid blocks]

            ...

            In total, 100 transactions were made and 50 blocks were validated
        */

        let mut stats = String::new();
        for (id, txs) in txs_sent {
            stats.push_str(&format!(
                "Peer {} made {} transactions and validated {} blocks\n",
                id,
                txs,
                blk_validated.get(&id).unwrap_or(&0),
            ));

            if itsx_sent.get(&id).is_some() || iblk_validated.get(&id).is_some() {
                stats.push_str(&format!(
                    "Peer {} made {} invalid transactions and validated {} invalid blocks\n",
                    id,
                    itsx_sent.get(&id).unwrap_or(&0),
                    iblk_validated.get(&id).unwrap_or(&0),
                ));
            }
        }

        stats.push_str(&format!(
            "In total, {} transactions were made and {} blocks were validated\n",
            total_tsx, toal_blk,
        ));

        if total_itsx > 0 || total_iblk > 0 {
            stats.push_str(&format!(
                "In total, {} invalid transactions were made and {} invalid blocks were validated\n",
                total_itsx, total_iblk,
            ));
        }

        stats
    }

    pub fn global_history() -> History {
        unsafe { GLOBAL_HISTORY.clone() }
    }

    pub fn log_local_transaction(tsx: &Transaction, peers: &PeersCatalog) {
        match tsx.payload() {
            TransactionPayload::Transfer(_) => Self::log_local_transfer(tsx, peers),
            TransactionPayload::Message(_) => Self::log_local_message(tsx, peers),
            TransactionPayload::Stake(_) => Self::log_local_stake(tsx, peers),
        }
    }

    fn log_local_transfer(tsx: &Transaction, peers: &PeersCatalog) {
        assert!(matches!(tsx.payload(), TransactionPayload::Transfer(_)));

        let src = peers
            .get_by_publ_key(tsx.sndr_addr().unwrap())
            .unwrap()
            .id();

        let event = Event {
            id: format!("T{}-{}", src, tsx.nonce()),
            src,
            dst: Some(
                peers
                    .get_by_publ_key(tsx.recp_addr().unwrap())
                    .unwrap()
                    .id(),
            ),
            kind: EventKind::LT {
                amount: tsx.payload().coins().unwrap() as f64 / CENTS_PER_COIN as f64,
            },
        };

        unsafe { GLOBAL_HISTORY.0.push(event) };
    }

    fn log_local_message(tsx: &Transaction, peers: &PeersCatalog) {
        assert!(matches!(tsx.payload(), TransactionPayload::Message(_)));

        let src = peers
            .get_by_publ_key(tsx.sndr_addr().unwrap())
            .unwrap()
            .id();

        let event = Event {
            id: format!("M{}-{}", src, tsx.nonce()),
            src,
            dst: Some(
                peers
                    .get_by_publ_key(tsx.recp_addr().unwrap())
                    .unwrap()
                    .id(),
            ),
            kind: EventKind::LM {
                message: tsx.payload().message().unwrap().to_string(),
            },
        };

        unsafe { GLOBAL_HISTORY.0.push(event) };
    }

    fn log_local_stake(tsx: &Transaction, peers: &PeersCatalog) {
        assert!(matches!(tsx.payload(), TransactionPayload::Stake(_)));

        let src = peers
            .get_by_publ_key(tsx.sndr_addr().unwrap())
            .unwrap()
            .id();

        let event = Event {
            id: format!("S{}-{}", src, tsx.nonce()),
            src,
            dst: None,
            kind: EventKind::LS {
                amount: tsx.payload().coins().unwrap() as f64 / CENTS_PER_COIN as f64,
            },
        };

        unsafe { GLOBAL_HISTORY.0.push(event) };
    }

    pub fn log_local_block(block: &Block, peers: &PeersCatalog) {
        let event = Event {
            id: format!("B{}", hex::encode(&block.hash()[..8])),
            src: peers.get_by_publ_key(block.val().unwrap()).unwrap().id(),
            dst: None,
            kind: EventKind::LB {
                tids: block
                    .tsxs()
                    .iter()
                    .map(|tsx| {
                        let src = peers
                            .get_by_publ_key(tsx.sndr_addr().unwrap())
                            .unwrap()
                            .id();
                        format!(
                            "{}{}-{}",
                            match tsx.payload() {
                                TransactionPayload::Transfer(_) => "T",
                                TransactionPayload::Message(_) => "M",
                                TransactionPayload::Stake(_) => "S",
                            },
                            src,
                            tsx.nonce()
                        )
                    })
                    .collect(),
            },
        };

        unsafe { GLOBAL_HISTORY.0.push(event) };
    }

    pub fn log_network_transaction(tsx: &Transaction, peers: &PeersCatalog) {
        match tsx.payload() {
            TransactionPayload::Transfer(_) => Self::log_network_transfer(tsx, peers),
            TransactionPayload::Message(_) => Self::log_network_message(tsx, peers),
            TransactionPayload::Stake(_) => Self::log_network_stake(tsx, peers),
        }
    }

    fn log_network_transfer(tsx: &Transaction, peers: &PeersCatalog) {
        assert!(matches!(tsx.payload(), TransactionPayload::Transfer(_)));

        let src = peers
            .get_by_publ_key(tsx.sndr_addr().unwrap())
            .unwrap()
            .id();

        let event = Event {
            id: format!("T{}-{}", src, tsx.nonce()),
            src,
            dst: Some(
                peers
                    .get_by_publ_key(tsx.recp_addr().unwrap())
                    .unwrap()
                    .id(),
            ),
            kind: EventKind::NT {
                amount: tsx.payload().coins().unwrap() as f64 / CENTS_PER_COIN as f64,
            },
        };

        unsafe { GLOBAL_HISTORY.0.push(event) };
    }

    fn log_network_message(tsx: &Transaction, peers: &PeersCatalog) {
        assert!(matches!(tsx.payload(), TransactionPayload::Message(_)));

        let src = peers
            .get_by_publ_key(tsx.sndr_addr().unwrap())
            .unwrap()
            .id();

        let event = Event {
            id: format!("M{}-{}", src, tsx.nonce()),
            src,
            dst: Some(
                peers
                    .get_by_publ_key(tsx.recp_addr().unwrap())
                    .unwrap()
                    .id(),
            ),
            kind: EventKind::NM {
                message: tsx.payload().message().unwrap().to_string(),
            },
        };

        unsafe { GLOBAL_HISTORY.0.push(event) };
    }

    fn log_network_stake(tsx: &Transaction, peers: &PeersCatalog) {
        assert!(matches!(tsx.payload(), TransactionPayload::Stake(_)));

        let src = peers
            .get_by_publ_key(tsx.sndr_addr().unwrap())
            .unwrap()
            .id();

        let event = Event {
            id: format!("S{}-{}", src, tsx.nonce()),
            src,
            dst: None,
            kind: EventKind::NS {
                amount: tsx.payload().coins().unwrap() as f64 / CENTS_PER_COIN as f64,
            },
        };

        unsafe { GLOBAL_HISTORY.0.push(event) };
    }

    pub fn log_network_block(block: &Block, peers: &PeersCatalog) {
        let event = Event {
            id: format!("B{}", hex::encode(&block.hash()[..8])),
            src: peers.get_by_publ_key(block.val().unwrap()).unwrap().id(),
            dst: None,
            kind: EventKind::NB {
                tids: block
                    .tsxs()
                    .iter()
                    .map(|tsx| {
                        let src = peers
                            .get_by_publ_key(tsx.sndr_addr().unwrap())
                            .unwrap()
                            .id();
                        format!(
                            "{}{}-{}",
                            match tsx.payload() {
                                TransactionPayload::Transfer(_) => "T",
                                TransactionPayload::Message(_) => "M",
                                TransactionPayload::Stake(_) => "S",
                            },
                            src,
                            tsx.nonce()
                        )
                    })
                    .collect(),
            },
        };

        unsafe { GLOBAL_HISTORY.0.push(event) };
    }

    pub fn log_invalid_transaction(tsx: &Transaction, peers: &PeersCatalog) {
        let src = peers
            .get_by_publ_key(tsx.sndr_addr().unwrap())
            .unwrap()
            .id();

        let event = Event {
            id: format!("IT{}-{}", src, tsx.nonce()),
            src,
            dst: Some(
                peers
                    .get_by_publ_key(tsx.recp_addr().unwrap())
                    .unwrap()
                    .id(),
            ),
            kind: EventKind::IT,
        };

        unsafe { GLOBAL_HISTORY.0.push(event) };
    }

    pub fn log_invalid_block(block: &Block, peers: &PeersCatalog) {
        let event = Event {
            id: format!("IB{}", hex::encode(&block.hash()[..8])),
            src: peers.get_by_publ_key(block.val().unwrap()).unwrap().id(),
            dst: None,
            kind: EventKind::IB,
        };

        unsafe { GLOBAL_HISTORY.0.push(event) };
    }

    pub fn log_new_validator(local_id: u32, vid: u32, blockchain: &Blockchain) {
        let event = Event {
            id: format!("V{}", blockchain.last_block().index()),
            src: local_id,
            dst: None,
            kind: EventKind::NV { vid },
        };

        unsafe { GLOBAL_HISTORY.0.push(event) };
    }
}

impl Display for History {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for event in &self.0 {
            match &event.kind {
                EventKind::LT { amount } => {
                    writeln!(
                        f,
                        "{} self to {} | {} BCC",
                        event.id,
                        event.dst.unwrap(),
                        amount
                    )?;
                }
                EventKind::LM { message } => {
                    writeln!(
                        f,
                        "{} self to {} | '{}'",
                        event.id,
                        event.dst.unwrap(),
                        message
                    )?;
                }
                EventKind::LS { amount } => {
                    writeln!(f, "{} self | {} BCC", event.id, amount)?;
                }
                EventKind::LB { tids } => {
                    writeln!(f, "{} by self | {:?}", event.id, tids)?;
                }
                EventKind::NT { amount } => {
                    writeln!(
                        f,
                        "{} {} to {} | {} BCC",
                        event.id,
                        event.src,
                        event.dst.unwrap(),
                        amount
                    )?;
                }
                EventKind::NM { message } => {
                    writeln!(
                        f,
                        "{} {} to {} | '{}'",
                        event.id,
                        event.src,
                        event.dst.unwrap(),
                        message
                    )?;
                }
                EventKind::NS { amount } => {
                    writeln!(f, "{} {} | {} BCC", event.id, event.src, amount)?;
                }
                EventKind::NB { tids } => {
                    writeln!(f, "{} by {} | {:?}", event.id, event.src, tids)?;
                }
                EventKind::IT => {
                    writeln!(f, "{} invalidated", event.id)?;
                }
                EventKind::IB => {
                    writeln!(f, "{} invalidated", event.id)?;
                }
                EventKind::NV { vid } => {
                    writeln!(f, "{} predicted {}", event.id, vid)?;
                }
            }
        }

        Ok(())
    }
}
