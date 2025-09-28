// main.rs - Main entry point for Alpenglow verification

mod types;
mod model;
mod tests;
mod benchmark;

use crate::tests::run_verification_suite;

fn main() {
    println!("Alpenglow Consensus Protocol Formal Verification");
    println!("Based on: Solana Alpenglow White Paper v1.1, July 22, 2025");
    println!("Authors: Quentin Kniep, Jakub Sliwinski, Roger Wattenhofer");
    println!("");
    
    // Run the complete verification suite
    let results = run_verification_suite();
    
    // Print final summary
    results.print_summary();
    
    // Additional analysis
    println!("\nDetailed Analysis:");
    println!("=================");
    
    analyze_protocol_properties(&results);
    provide_verification_insights(&results);
    
    println!("\nVerification completed. See results above.");
}

fn analyze_protocol_properties(results: &tests::VerificationResults) {
    println!("\nProtocol Properties Analysis:");
    
    // Check core safety properties
    let safety_passed = results.results.get("safety_agreement")
        .map(|s| matches!(s, stateright::CheckerStatus::Pass))
        .unwrap_or(false) &&
        results.results.get("safety_validity")
        .map(|s| matches!(s, stateright::CheckerStatus::Pass))
        .unwrap_or(false);
    
    if safety_passed {
        println!("✅ SAFETY: Agreement and Validity properties verified");
        println!("   - No two correct nodes finalize conflicting blocks");
        println!("   - Only designated leader blocks can be finalized");
    } else {
        println!("❌ SAFETY: Critical safety properties failed verification");
    }
    
    // Check liveness
    let liveness_passed = results.results.get("liveness")
        .map(|s| matches!(s, stateright::CheckerStatus::Pass))
        .unwrap_or(false);
    
    if liveness_passed {
        println!("✅ LIVENESS: Progress property verified");
        println!("   - System makes progress in synchronous periods");
    } else {
        println!("⚠️  LIVENESS: May have issues under certain conditions");
    }
    
    // Check fault tolerance
    let fault_tolerance_passed = results.results.get("byzantine_bound")
        .map(|s| matches!(s, stateright::CheckerStatus::Pass))
        .unwrap_or(false);
    
    if fault_tolerance_passed {
        println!("✅ FAULT TOLERANCE: 20% Byzantine bound verified");
        println!("   - Protocol remains safe with ≤20% Byzantine stake");
    } else {
        println!("❌ FAULT TOLERANCE: Issues with Byzantine fault handling");
    }
}

fn provide_verification_insights(results: &tests::VerificationResults) {
    println!("\nVerification Insights:");
    
    // Analyze finalization paths
    let fast_path = results.results.get("fast_finalization");
    let slow_path = results.results.get("slow_finalization");
    
    match (fast_path, slow_path) {
        (Some(stateright::CheckerStatus::Pass), Some(stateright::CheckerStatus::Pass)) => {
            println!("✅ Both fast (80%) and slow (60%) finalization paths work correctly");
            println!("   - Fast path: Single round with 80% stake participation");
            println!("   - Slow path: Two rounds with 60% stake participation");
        }
        _ => {
            println!("⚠️  Finalization path issues detected - review voting logic");
        }
    }
    
    // Analyze scalability
    let mut scalability_issues = Vec::new();
    for (test_name, status) in &results.results {
        if test_name.starts_with("scalability_") && !matches!(status, stateright::CheckerStatus::Pass) {
            scalability_issues.push(test_name);
        }
    }
    
    if scalability_issues.is_empty() {
        println!("✅ Protocol scales well across different network sizes");
    } else {
        println!("⚠️  Scalability issues found in: {:?}", scalability_issues);
    }
    
    // Check leader rotation
    if let Some(leader_status) = results.results.get("leader_rotation") {
        match leader_status {
            stateright::CheckerStatus::Pass => {
                println!("✅ Leader rotation mechanism works correctly");
            }
            _ => {
                println!("❌ Leader rotation has issues - may affect liveness");
            }
        }
    }
}

// Additional utility functions for detailed analysis
#[allow(dead_code)]
fn analyze_state_space_characteristics() {
    println!("\nState Space Analysis:");
    println!("====================");
    
    let (total_states, unique_states) = tests::performance::analyze_state_space();
    println!("Total states explored: {}", total_states);
    println!("Unique states: {}", unique_states);
    
    let efficiency = (unique_states as f64 / total_states as f64) * 100.0;
    println!("State space efficiency: {:.1}%", efficiency);
    
    if efficiency > 80.0 {
        println!("✅ Efficient state exploration");
    } else if efficiency > 60.0 {
        println!("⚠️  Moderate state explosion detected");
    } else {
        println!("❌ Significant state explosion - consider abstraction");
    }
}

#[allow(dead_code)]
fn generate_verification_report(results: &tests::VerificationResults) -> String {
    let mut report = String::new();
    
    report.push_str("# Alpenglow Consensus Protocol Verification Report\n\n");
    report.push_str("## Summary\n");
    report.push_str(&format!("- Total Tests: {}\n", results.total));
    report.push_str(&format!("- Passed: {}\n", results.passed));
    report.push_str(&format!("- Failed: {}\n", results.failed));
    report.push_str(&format!("- Success Rate: {:.1}%\n\n", 
        (results.passed as f64 / results.total as f64) * 100.0));
    
    report.push_str("## Key Findings\n");
    
    // Safety analysis
    let safety_results = results.results.iter()
        .filter(|(name, _)| name.contains("safety"))
        .collect::<Vec<_>>();
    
    report.push_str("### Safety Properties\n");
    for (name, status) in safety_results {
        let status_str = match status {
            stateright::CheckerStatus::Pass => "✅ VERIFIED",
            _ => "❌ FAILED",
        };
        report.push_str(&format!("- {}: {}\n", name, status_str));
    }
    
    // Liveness analysis  
    if let Some(liveness_status) = results.results.get("liveness") {
        report.push_str("\n### Liveness Properties\n");
        let status_str = match liveness_status {
            stateright::CheckerStatus::Pass => "✅ VERIFIED",
            _ => "❌ FAILED", 
        };
        report.push_str(&format!("- Liveness: {}\n", status_str));
    }
    
    // Fault tolerance analysis
    report.push_str("\n### Fault Tolerance\n");
    let fault_results = results.results.iter()
        .filter(|(name, _)| name.contains("fault") || name.contains("byzantine"))
        .collect::<Vec<_>>();
        
    for (name, status) in fault_results {
        let status_str = match status {
            stateright::CheckerStatus::Pass => "✅ VERIFIED",
            _ => "❌ FAILED",
        };
        report.push_str(&format!("- {}: {}\n", name, status_str));
    }
    
    report.push_str("\n## Detailed Results\n");
    for (name, status) in &results.results {
        report.push_str(&format!("- {}: {:?}\n", name, status));
    }
    
    report
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[test] 
    fn test_basic_model_creation() {
        let model = model::AlpenglowModel::new(5, 1);
        let init_states = model.init_states();
        assert_eq!(init_states.len(), 1);
        
        let state = &init_states[0];
        assert_eq!(state.nodes.len(), 5);
        assert_eq!(state.byzantine_stake_percentage(), 20);
    }
    
    #[test]
    fn test_safety_properties_hold() {
        let model = model::AlpenglowModel::new(5, 1);
        let init_state = &model.init_states()[0];
        
        // Test initial state satisfies safety properties
        assert!(types::safety_property_agreement(init_state));
        assert!(types::safety_property_validity(init_state));
        assert!(types::fault_tolerance_property(init_state));
    }
    
    #[test]
    fn test_byzantine_assumption() {
        let model = model::AlpenglowModel::new(10, 2);
        let init_state = &model.init_states()[0];
        
        // Should be exactly at the 20% bound
        assert_eq!(init_state.byzantine_stake_percentage(), 20);
        assert!(types::fault_tolerance_property(init_state));
    }
    
    #[test]
    fn test_leader_schedule() {
        let model = model::AlpenglowModel::new(4, 0);
        let init_state = &model.init_states()[0];
        
        // Test that leader schedule is properly set up
        for slot in 1..=8 {
            let leader = init_state.get_leader(types::SlotNumber(slot));
            assert!(leader.is_some());
            assert!(leader.unwrap().0 < 4);
        }
    }
}