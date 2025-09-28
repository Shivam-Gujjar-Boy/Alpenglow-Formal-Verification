use alpenglow_verification::{AlpenglowModel, run_verification_suite};

fn main() {
    println!("Running basic Alpenglow verification example...");
    
    // Create a simple 3-node network with no Byzantine nodes
    let model = AlpenglowModel::new(3, 0);
    
    println!("Model created with 3 nodes, 0 Byzantine");
    println!("Starting verification...");
    
    // Run the full verification suite
    let results = run_verification_suite();
    results.print_summary();
}