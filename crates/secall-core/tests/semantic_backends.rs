#[test]
fn semantic_backend_regressions_live_in_unit_tests() {
    // Backend dispatch regressions moved into graph::semantic unit tests so
    // helper visibility can stay pub(crate). This integration target is kept
    // intentionally as a stub, rather than deleted, so the documented
    // verification command still resolves to a valid test target.
}
