pub mod block;
pub mod transaction;

use self::block::Block;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Blockchain {
    blocks: Vec<Block>,
}

impl Blockchain {
    pub fn new(gen_blk: Block) -> Self {
        Self {
            blocks: vec![gen_blk],
        }
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    pub fn add_block(&mut self, mut blk: Block) {
        blk.index = self.blocks.len() as u32;
        self.blocks.push(blk);
    }

    // the blockchain will always have at least one block
    // so this helps avoid unwrapping every time we need the last block
    pub fn last_block(&self) -> &Block {
        self.blocks.last().unwrap()
    }
}
