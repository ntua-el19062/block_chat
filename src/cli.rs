/*
    This module contains a struct representing the CLI of BlockChat, along with
    a struct representing the available commands. These structs are created using the
    crate `clap`. See its documentation to understand the syntax.
*/

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::{
    fmt::{self, Display, Formatter},
    num::NonZeroU32,
};

#[derive(Debug, Deserialize, Parser, Serialize)]
pub struct Args {
    #[command(name = "command", subcommand)]
    pub cmd: Command,
}

impl Display for Args {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Command: {}", self.cmd)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Subcommand)]
pub enum Command {
    /// Send BCC to another user
    #[command(arg_required_else_help = true)]
    T {
        /// The network ID of the recipient
        #[arg(name = "RECIPIENT_ID")]
        rcp_id: u32,
        /// The amount of BCC to send
        #[arg(name = "AMOUNT")]
        amt: NonZeroU32,
    },

    /// Send a message to another user
    #[command(arg_required_else_help = true)]
    M {
        /// The network ID of the recipient
        #[arg(name = "RECIPIENT_ID")]
        rcp_id: u32,
        /// The message to send
        #[arg(name = "MESSAGE")]
        msg: Vec<String>,
    },

    /// Stake BCC to verify transactions
    #[command(name = "stake", arg_required_else_help = true)]
    S {
        /// The amount of BCC to stake
        #[arg(name = "AMOUNT")]
        amt: NonZeroU32,
    },

    /// View all transactions of the last verified block
    #[command(name = "view")]
    V,

    /// View your current BCC balance
    #[command(name = "balance")]
    B,

    // * debug only
    /// View the history of transactions and blocks
    #[command(name = "history")]
    H,

    // * debug only
    /// View your network ID
    #[command(skip = true)]
    Id,

    // * debug only
    /// View the average time per transaction and block
    Time,

    // * debug only
    /// View the stats of the network (transactions and blocks per node)
    Stats,
}

impl Display for Command {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Command::T { rcp_id, amt } => write!(f, "t {} {}", rcp_id, amt),
            Command::M { rcp_id, msg } => write!(f, "m {} {}", rcp_id, msg.join(" ")),
            Command::S { amt } => write!(f, "stake {}", amt),
            Command::V => write!(f, "view"),
            Command::B => write!(f, "balance"),
            Command::H => write!(f, "history"),
            Command::Id => write!(f, "id"),
            Command::Time => write!(f, "time"),
            Command::Stats => write!(f, "stats"),
        }
    }
}
