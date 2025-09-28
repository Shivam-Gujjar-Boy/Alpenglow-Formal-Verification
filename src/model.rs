use stateright::*;
use std::collections::{HashMap, HashSet};
use crate::types::*; // Assuming the types are in a separate module

// The main Alpenglow consensus model
#[derive(Clone)]
pub struct AlpenglowModel {
    pub node_count: usize,
    pub byzantine_count: usize,
    pub max_slots: u64,
}

impl AlpenglowModel {
    pub fn new(node_count: usize, byzantine_count: usize) -> Self {
        AlpenglowModel {
            node_count,
            byzantine_count,
            max_slots: 10, // Keep small for verification
        }
    }

    // Helper function to calculate stake weight for a set of nodes
    fn calculate_stake_weight(&self, state: &AlpenglowState, voters: &HashSet<NodeId>) -> u8 {
        voters.iter()
            .map(|node_id| {
                state.nodes.get(node_id.0)
                    .map(|node| node.stake.percentage())
                    .unwrap_or(0)
            })
            .sum::<u8>()
            .min(100)
    }

    // Check if enough votes exist to form a certificate
    fn can_form_certificate(&self, state: &AlpenglowState, slot: SlotNumber, 
                           vote_type: &VoteType, threshold: u8) -> Option<Certificate> {
        let mut supporting_nodes = HashSet::new();
        
        // Collect all nodes that cast the specific vote type
        for node in &state.nodes {
            if let Some(votes) = node.pool_votes.get(&slot) {
                for vote in votes {
                    match (&vote.vote_type, vote_type) {
                        (VoteType::Notarization(h1), VoteType::Notarization(h2)) if h1 == h2 => {
                            supporting_nodes.insert(vote.voter.clone());
                        }
                        (VoteType::Skip(s1), VoteType::Skip(s2)) if s1 == s2 => {
                            supporting_nodes.insert(vote.voter.clone());
                        }
                        (VoteType::Finalization(s1), VoteType::Finalization(s2)) if s1 == s2 => {
                            supporting_nodes.insert(vote.voter.clone());
                        }
                        _ => {}
                    }
                }
            }
        }

        let total_stake = self.calculate_stake_weight(state, &supporting_nodes);
        if total_stake >= threshold {
            let cert_type = match vote_type {
                VoteType::Notarization(hash) => {
                    if total_stake >= 80 {
                        CertificateType::FastFinalization(hash.clone())
                    } else {
                        CertificateType::Notarization(hash.clone())
                    }
                }
                VoteType::Skip(slot_num) => CertificateType::Skip(*slot_num),
                VoteType::Finalization(slot_num) => CertificateType::Finalization(*slot_num),
                _ => return None,
            };

            Some(Certificate {
                cert_type,
                slot,
                supporting_stake: Stake(total_stake),
                voters: supporting_nodes,
            })
        } else {
            None
        }
    }

    // Determine what vote a correct node should cast
    fn determine_correct_vote(&self, state: &AlpenglowState, node_id: NodeId, slot: SlotNumber) 
        -> Option<VoteType> {
        let node = &state.nodes[node_id.0];
        
        // Don't vote if already voted in this slot
        if node.has_voted_in_slot(slot) {
            return None;
        }

        // Check if there's a block received for this slot
        if let Some(block) = node.received_blocks.get(&slot) {
            // Check if parent is available and valid
            if slot.0 == 1 || self.is_parent_ready(state, node_id, block) {
                return Some(VoteType::Notarization(block.hash.clone()));
            }
        }

        // If no valid block or parent not ready, vote to skip
        Some(VoteType::Skip(slot))
    }

    // Check if parent block is ready for building upon
    fn is_parent_ready(&self, state: &AlpenglowState, node_id: NodeId, block: &Block) -> bool {
        let node = &state.nodes[node_id.0];
        
        if let Some(parent_hash) = &block.parent_hash {
            // Check if parent is notarized
            node.is_block_notarized(parent_hash)
        } else {
            // Genesis block case
            block.slot.0 == 0
        }
    }

    // Check if a block can be finalized
    fn can_finalize_block(&self, state: &AlpenglowState, node_id: NodeId, 
                         block_hash: &BlockHash) -> bool {
        let node = &state.nodes[node_id.0];
        
        // Check for fast-finalization certificate (80% notarization votes)
        for cert in &node.observed_certificates {
            if let CertificateType::FastFinalization(hash) = &cert.cert_type {
                if hash == block_hash {
                    return true;
                }
            }
        }

        // Check for slow-finalization (60% finalization votes after notarization)
        let has_notarization = node.observed_certificates.iter().any(|cert| {
            matches!(cert.cert_type, CertificateType::Notarization(ref hash) if hash == block_hash)
        });
        
        let has_finalization = node.observed_certificates.iter().any(|cert| {
            if let CertificateType::Finalization(slot) = &cert.cert_type {
                // Check if this finalization corresponds to our block
                if let Some(block) = node.received_blocks.values().find(|b| b.hash == *block_hash) {
                    return block.slot == *slot;
                }
            }
            false
        });

        has_notarization && has_finalization
    }
}

impl Model for AlpenglowModel {
    type State = AlpenglowState;
    type Action = AlpenglowAction;

    fn init_states(&self) -> Vec<Self::State> {
        vec![AlpenglowState::new(self.node_count, self.byzantine_count)]
    }

    fn actions(&self, state: &Self::State, actions: &mut Vec<Self::Action>) {
        // Bound the state space by limiting slot advancement
        if state.global_slot.0 >= self.max_slots {
            return;
        }

        // 1. Leader can propose a block
        if let Some(leader_id) = state.get_leader(state.global_slot) {
            let leader = &state.nodes[leader_id.0];
            if leader.behavior == NodeBehavior::Correct {
                actions.push(AlpenglowAction::ProposeBlock {
                    leader: leader_id,
                    slot: state.global_slot,
                });
            }
        }

        // 2. Nodes can cast votes
        for (i, node) in state.nodes.iter().enumerate() {
            if node.behavior == NodeBehavior::Correct {
                let node_id = NodeId(i);
                
                // Vote in current slot
                if let Some(vote_type) = self.determine_correct_vote(state, node_id, state.global_slot) {
                    actions.push(AlpenglowAction::CastVote {
                        voter: node_id,
                        vote: Vote {
                            voter: node_id,
                            slot: state.global_slot,
                            vote_type,
                        },
                    });
                }

                // Cast finalization votes for notarized blocks
                for cert in &node.observed_certificates {
                    if let CertificateType::Notarization(block_hash) = &cert.cert_type {
                        if !node.has_voted_in_slot(cert.slot) || 
                           !matches!(node.get_vote_in_slot(cert.slot).map(|v| &v.vote_type), 
                                    Some(VoteType::Finalization(_))) {
                            actions.push(AlpenglowAction::CastVote {
                                voter: node_id,
                                vote: Vote {
                                    voter: node_id,
                                    slot: cert.slot,
                                    vote_type: VoteType::Finalization(cert.slot),
                                },
                            });
                        }
                    }
                }
            }
        }

        // 3. Form certificates when enough votes are collected
        for slot_num in 1..=state.global_slot.0 {
            let slot = SlotNumber(slot_num);
            
            // Check for notarization certificates
            for node in &state.nodes {
                if let Some(votes) = node.pool_votes.get(&slot) {
                    let mut notarization_votes: HashMap<BlockHash, HashSet<NodeId>> = HashMap::new();
                    
                    for vote in votes {
                        if let VoteType::Notarization(block_hash) = &vote.vote_type {
                            notarization_votes.entry(block_hash.clone())
                                .or_insert_with(HashSet::new)
                                .insert(vote.voter.clone());
                        }
                    }

                    for (block_hash, voters) in notarization_votes {
                        let stake = self.calculate_stake_weight(state, &voters);
                        if stake >= 60 {
                            let cert_type = if stake >= 80 {
                                CertificateType::FastFinalization(block_hash)
                            } else {
                                CertificateType::Notarization(block_hash)
                            };

                            actions.push(AlpenglowAction::CreateCertificate {
                                node: NodeId(0), // Any node can observe this
                                cert_type,
                                slot,
                            });
                        }
                    }
                }
            }
        }

        // 4. Finalize blocks with sufficient certificates
        for (i, node) in state.nodes.iter().enumerate() {
            if node.behavior == NodeBehavior::Correct {
                let node_id = NodeId(i);
                
                for block in node.received_blocks.values() {
                    if !node.is_block_finalized(&block.hash) && 
                       self.can_finalize_block(state, node_id, &block.hash) {
                        actions.push(AlpenglowAction::FinalizeBlock {
                            node: node_id,
                            block_hash: block.hash.clone(),
                        });
                    }
                }
            }
        }

        // 5. Advance to next slot
        actions.push(AlpenglowAction::AdvanceSlot);
    }

    fn next_state(&self, state: &Self::State, action: Self::Action) -> Option<Self::State> {
        let mut new_state = state.clone();

        match action {
            AlpenglowAction::ProposeBlock { leader, slot } => {
                // Leader proposes a block
                let parent_hash = if slot.0 == 1 {
                    Some(BlockHash("genesis".to_string()))
                } else {
                    // Find the most recent finalized block
                    state.nodes[leader.0].finalized_blocks
                        .last()
                        .map(|b| b.hash.clone())
                };

                let block = Block {
                    slot,
                    hash: BlockHash(format!("block_{}_{}", slot.0, leader.0)),
                    parent_hash,
                    leader,
                };

                // Deliver block to all nodes (in synchronous network)
                if new_state.network_condition == NetworkCondition::Synchronous {
                    for node in &mut new_state.nodes {
                        if node.behavior != NodeBehavior::Crashed {
                            node.received_blocks.insert(slot, block.clone());
                        }
                    }
                }
            }

            AlpenglowAction::CastVote { voter, vote } => {
                // Node casts a vote
                if let Some(node) = new_state.nodes.get_mut(voter.0) {
                    if !node.has_voted_in_slot(vote.slot) {
                        node.add_vote(vote.clone());
                        
                        // Broadcast vote to all other nodes
                        for other_node in &mut new_state.nodes {
                            if other_node.id != voter && other_node.behavior != NodeBehavior::Crashed {
                                other_node.observe_vote(vote.clone());
                            }
                        }
                    }
                }
            }

            AlpenglowAction::CreateCertificate { node, cert_type, slot } => {
                // Create and broadcast certificate
                let cert = Certificate {
                    cert_type,
                    slot,
                    supporting_stake: Stake(60), // Simplified
                    voters: HashSet::new(), // Simplified
                };

                for node_state in &mut new_state.nodes {
                    if node_state.behavior != NodeBehavior::Crashed {
                        if !node_state.observed_certificates.contains(&cert) {
                            node_state.observed_certificates.push(cert.clone());
                        }
                    }
                }
            }

            AlpenglowAction::FinalizeBlock { node, block_hash } => {
                // Node finalizes a block
                if let Some(node_state) = new_state.nodes.get_mut(node.0) {
                    if let Some(block) = node_state.received_blocks.values()
                        .find(|b| b.hash == block_hash).cloned() {
                        
                        if !node_state.is_block_finalized(&block_hash) {
                            // Also finalize all ancestors
                            let mut to_finalize = vec![block];
                            while let Some(current_block) = to_finalize.last() {
                                if let Some(parent_hash) = &current_block.parent_hash {
                                    if let Some(parent) = node_state.received_blocks.values()
                                        .find(|b| b.hash == *parent_hash).cloned() {
                                        if !node_state.is_block_finalized(parent_hash) {
                                            to_finalize.push(parent);
                                        } else {
                                            break;
                                        }
                                    } else {
                                        break;
                                    }
                                } else {
                                    break;
                                }
                            }

                            // Finalize in reverse order (ancestors first)
                            for block_to_finalize in to_finalize.into_iter().rev() {
                                if !node_state.is_block_finalized(&block_to_finalize.hash) {
                                    node_state.finalized_blocks.push(block_to_finalize);
                                }
                            }
                        }
                    }
                }
            }

            AlpenglowAction::AdvanceSlot => {
                new_state.global_slot.0 += 1;
                for node in &mut new_state.nodes {
                    node.current_slot.0 += 1;
                }
            }

            _ => {} // Other actions not implemented yet
        }

        Some(new_state)
    }

    fn properties(&self) -> Vec<Property<Self>> {
        vec![
            Property::always("safety_agreement", |state: &AlpenglowState| {
                safety_property_agreement(state)
            }),
            Property::always("safety_validity", |state: &AlpenglowState| {
                safety_property_validity(state)
            }),
            Property::always("fault_tolerance", |state: &AlpenglowState| {
                fault_tolerance_property(state)
            }),
            Property::eventually("liveness", |state: &AlpenglowState| {
                liveness_property(state)
            }),
            // Additional property: Byzantine nodes don't exceed 20%
            Property::always("byzantine_assumption", |state: &AlpenglowState| {
                state.byzantine_stake_percentage() <= 20
            }),
        ]
    }
}

// Checker configuration for different scenarios
pub fn create_basic_checker() -> Checker<AlpenglowModel> {
    // 5 nodes, 1 Byzantine (20%)
    let model = AlpenglowModel::new(5, 1);
    Checker::spawn(model)
        .visitor(stateright::explorer::BfsVisitor::new())
        .threads(1)
}

pub fn create_stress_test_checker() -> Checker<AlpenglowModel> {
    // Test boundary case: 5 nodes, 1 Byzantine (exactly 20%)
    let model = AlpenglowModel::new(5, 1);
    Checker::spawn(model)
        .visitor(stateright::explorer::DfsVisitor::new())
        .threads(2)
}

pub fn create_large_network_checker() -> Checker<AlpenglowModel> {
    // Larger network: 10 nodes, 2 Byzantine (20%)
    let model = AlpenglowModel::new(10, 2);
    Checker::spawn(model)
        .visitor(stateright::explorer::BfsVisitor::new())
        .threads(4)
        .max_depth(15) // Limit depth due to state explosion
}

// Helper function to run verification with different network conditions
pub fn verify_under_network_conditions() -> Vec<(NetworkCondition, stateright::CheckerStatus)> {
    let mut results = Vec::new();
    
    for condition in [NetworkCondition::Synchronous, NetworkCondition::Asynchronous] {
        let mut model = AlpenglowModel::new(5, 1);
        let status = Checker::spawn(model)
            .visitor(stateright::explorer::BfsVisitor::new())
            .threads(1)
            .max_depth(10)
            .check();
        
        results.push((condition, status));
    }
    
    results
}