use stateright::*;
use std::collections::{HashMap, HashSet, BTreeMap};
use std::hash::{Hash, Hasher};

// Core identifiers
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NodeId(pub usize);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SlotNumber(pub u64);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BlockHash(pub String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EpochNumber(pub u64);

// Stake representation (percentage as integer 0-100)
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Stake(pub u8);

impl Stake {
    pub fn percentage(&self) -> u8 { self.0 }
    pub fn is_majority_60(&self) -> bool { self.0 >= 60 }
    pub fn is_supermajority_80(&self) -> bool { self.0 >= 80 }
}

// Block structure simplified for verification
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Block {
    pub slot: SlotNumber,
    pub hash: BlockHash,
    pub parent_hash: Option<BlockHash>,
    pub leader: NodeId,
}

impl Block {
    pub fn genesis() -> Self {
        Block {
            slot: SlotNumber(0),
            hash: BlockHash("genesis".to_string()),
            parent_hash: None,
            leader: NodeId(0),
        }
    }
}

// Vote types from the protocol
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum VoteType {
    Notarization(BlockHash),
    NotarFallback(BlockHash),
    Skip(SlotNumber),
    SkipFallback(SlotNumber), 
    Finalization(SlotNumber),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Vote {
    pub voter: NodeId,
    pub slot: SlotNumber,
    pub vote_type: VoteType,
}

// Certificate types
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CertificateType {
    FastFinalization(BlockHash), // 80% notarization votes
    Notarization(BlockHash),     // 60% notarization votes  
    NotarFallback(BlockHash),    // 60% notar or notar-fallback votes
    Skip(SlotNumber),            // 60% skip or skip-fallback votes
    Finalization(SlotNumber),    // 60% finalization votes
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Certificate {
    pub cert_type: CertificateType,
    pub slot: SlotNumber,
    pub supporting_stake: Stake,
    pub voters: HashSet<NodeId>,
}

// Node behavior types
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum NodeBehavior {
    Correct,
    Byzantine,
    Crashed,
}

// Network conditions
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum NetworkCondition {
    Synchronous,
    Asynchronous,
    Partitioned,
}

// Node state in the protocol
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct NodeState {
    pub id: NodeId,
    pub stake: Stake,
    pub behavior: NodeBehavior,
    pub current_slot: SlotNumber,
    pub received_blocks: HashMap<SlotNumber, Block>,
    pub cast_votes: HashMap<SlotNumber, Vote>,
    pub observed_certificates: Vec<Certificate>,
    pub finalized_blocks: Vec<Block>,
    pub pool_votes: HashMap<SlotNumber, Vec<Vote>>, // Votes observed from others
}

impl NodeState {
    pub fn new(id: NodeId, stake: Stake, behavior: NodeBehavior) -> Self {
        let mut node = NodeState {
            id,
            stake,
            behavior,
            current_slot: SlotNumber(1),
            received_blocks: HashMap::new(),
            cast_votes: HashMap::new(),
            observed_certificates: Vec::new(),
            finalized_blocks: Vec::new(),
            pool_votes: HashMap::new(),
        };
        
        // Add genesis block
        let genesis = Block::genesis();
        node.received_blocks.insert(SlotNumber(0), genesis.clone());
        node.finalized_blocks.push(genesis);
        
        node
    }

    pub fn has_voted_in_slot(&self, slot: SlotNumber) -> bool {
        self.cast_votes.contains_key(&slot)
    }

    pub fn get_vote_in_slot(&self, slot: SlotNumber) -> Option<&Vote> {
        self.cast_votes.get(&slot)
    }

    pub fn add_vote(&mut self, vote: Vote) {
        self.cast_votes.insert(vote.slot, vote);
    }

    pub fn observe_vote(&mut self, vote: Vote) {
        self.pool_votes.entry(vote.slot)
            .or_insert_with(Vec::new)
            .push(vote);
    }

    pub fn get_stake_for_vote_type(&self, slot: SlotNumber, vote_type: &VoteType) -> Stake {
        let mut total_stake = 0u8;
        
        if let Some(votes) = self.pool_votes.get(&slot) {
            for vote in votes {
                if vote.vote_type == *vote_type {
                    // In a real implementation, we'd look up the voter's stake
                    // For simplicity, assuming equal stake distribution
                    total_stake += 10; // Simplified
                }
            }
        }
        
        Stake(total_stake.min(100))
    }

    pub fn is_block_notarized(&self, block_hash: &BlockHash) -> bool {
        self.observed_certificates.iter().any(|cert| {
            matches!(cert.cert_type, CertificateType::Notarization(ref hash) if hash == block_hash) ||
            matches!(cert.cert_type, CertificateType::FastFinalization(ref hash) if hash == block_hash)
        })
    }

    pub fn is_block_finalized(&self, block_hash: &BlockHash) -> bool {
        self.finalized_blocks.iter().any(|block| &block.hash == block_hash)
    }
}

// Global system state
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AlpenglowState {
    pub nodes: Vec<NodeState>,
    pub network_condition: NetworkCondition,
    pub current_epoch: EpochNumber,
    pub total_stake: Stake,
    pub leader_schedule: HashMap<SlotNumber, NodeId>,
    pub global_slot: SlotNumber,
}

impl AlpenglowState {
    pub fn new(node_count: usize, byzantine_count: usize) -> Self {
        let mut nodes = Vec::new();
        let stake_per_node = 100 / node_count as u8;
        
        for i in 0..node_count {
            let behavior = if i < byzantine_count {
                NodeBehavior::Byzantine
            } else {
                NodeBehavior::Correct
            };
            
            nodes.push(NodeState::new(
                NodeId(i),
                Stake(stake_per_node),
                behavior,
            ));
        }

        // Simple round-robin leader schedule
        let mut leader_schedule = HashMap::new();
        for slot in 1..=100 {
            leader_schedule.insert(
                SlotNumber(slot),
                NodeId((slot - 1) as usize % node_count),
            );
        }

        AlpenglowState {
            nodes,
            network_condition: NetworkCondition::Synchronous,
            current_epoch: EpochNumber(1),
            total_stake: Stake(100),
            leader_schedule,
            global_slot: SlotNumber(1),
        }
    }

    pub fn get_leader(&self, slot: SlotNumber) -> Option<NodeId> {
        self.leader_schedule.get(&slot).cloned()
    }

    pub fn get_correct_nodes(&self) -> Vec<&NodeState> {
        self.nodes.iter()
            .filter(|node| node.behavior == NodeBehavior::Correct)
            .collect()
    }

    pub fn get_byzantine_nodes(&self) -> Vec<&NodeState> {
        self.nodes.iter()
            .filter(|node| node.behavior == NodeBehavior::Byzantine)
            .collect()
    }

    pub fn byzantine_stake_percentage(&self) -> u8 {
        self.get_byzantine_nodes()
            .iter()
            .map(|node| node.stake.percentage())
            .sum()
    }

    pub fn crashed_stake_percentage(&self) -> u8 {
        self.nodes.iter()
            .filter(|node| node.behavior == NodeBehavior::Crashed)
            .map(|node| node.stake.percentage())
            .sum()
    }
}

// Messages that can be sent between nodes
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Message {
    Block(Block),
    Vote(Vote),
    Certificate(Certificate),
    Timeout(SlotNumber),
}

// Actions that can be performed in the system
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum AlpenglowAction {
    ProposeBlock {
        leader: NodeId,
        slot: SlotNumber,
    },
    CastVote {
        voter: NodeId, 
        vote: Vote,
    },
    DeliverMessage {
        from: NodeId,
        to: NodeId,
        message: Message,
    },
    AdvanceSlot,
    TimeoutSlot {
        node: NodeId,
        slot: SlotNumber,
    },
    CreateCertificate {
        node: NodeId,
        cert_type: CertificateType,
        slot: SlotNumber,
    },
    FinalizeBlock {
        node: NodeId,
        block_hash: BlockHash,
    },
}

// Safety properties we want to verify
pub fn safety_property_agreement(state: &AlpenglowState) -> bool {
    // No two correct nodes finalize conflicting blocks in the same slot
    let correct_nodes = state.get_correct_nodes();
    
    for slot_num in 1..=state.global_slot.0 {
        let slot = SlotNumber(slot_num);
        let mut finalized_in_slot: HashSet<BlockHash> = HashSet::new();
        
        for node in &correct_nodes {
            for block in &node.finalized_blocks {
                if block.slot == slot {
                    if finalized_in_slot.contains(&block.hash) {
                        continue; // Same block, OK
                    } else if !finalized_in_slot.is_empty() {
                        return false; // Conflicting blocks finalized
                    }
                    finalized_in_slot.insert(block.hash.clone());
                }
            }
        }
    }
    
    true
}

pub fn safety_property_validity(state: &AlpenglowState) -> bool {
    // Only blocks proposed by designated leaders can be finalized
    let correct_nodes = state.get_correct_nodes();
    
    for node in &correct_nodes {
        for block in &node.finalized_blocks {
            if block.slot.0 > 0 { // Skip genesis
                if let Some(designated_leader) = state.get_leader(block.slot) {
                    if block.leader != designated_leader {
                        return false;
                    }
                } else {
                    return false; // No leader designated
                }
            }
        }
    }
    
    true
}

pub fn liveness_property(state: &AlpenglowState) -> bool {
    // In synchronous periods, progress should be made
    if state.network_condition == NetworkCondition::Synchronous {
        let correct_nodes = state.get_correct_nodes();
        if !correct_nodes.is_empty() {
            // At least one correct node should have finalized recent blocks
            let latest_finalized = correct_nodes.iter()
                .map(|node| {
                    node.finalized_blocks.iter()
                        .map(|block| block.slot.0)
                        .max()
                        .unwrap_or(0)
                })
                .max()
                .unwrap_or(0);
                
            // Should be making progress (not stuck too far behind)
            return state.global_slot.0 - latest_finalized <= 5;
        }
    }
    true
}

// Fault tolerance property - system should remain safe with up to 20% Byzantine nodes
pub fn fault_tolerance_property(state: &AlpenglowState) -> bool {
    let byzantine_percentage = state.byzantine_stake_percentage();
    if byzantine_percentage <= 20 {
        // Safety should hold
        safety_property_agreement(state) && safety_property_validity(state)
    } else {
        // No guarantees if more than 20% Byzantine
        true
    }
}