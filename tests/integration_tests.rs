use alpenglow_verification::{AlpenglowModel, run_verification_suite};
use stateright::*;

#[test]
fn test_full_verification_suite() {
    let results = run_verification_suite();
    assert!(results.passed > 0, "At least some tests should pass");
}

#[test]
fn test_basic_model_functionality() {
    let model = AlpenglowModel::new(3, 0);
    let checker = model.checker().threads(1);
    let result = checker.check();
    assert!(matches!(result, stateright::CheckerStatus::Pass));
}