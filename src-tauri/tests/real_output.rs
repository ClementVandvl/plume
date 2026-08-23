//! Parses a real `structured_output` payload captured from `claude -p`.
//!
//! The unit tests use hand-written samples; this one guards against the model
//! emitting a shape we did not anticipate.

use plume_lib::ir::Block;

#[test]
fn real_model_output_parses() {
    let raw = include_str!("real_output.json");
    let blocks: Vec<Block> = serde_json::from_str(raw).expect("captured output must parse");
    assert!(!blocks.is_empty());
    for block in &blocks {
        assert!(!block.kind.is_empty());
        assert!(block.confidence > 0.0);
    }
    eprintln!("{} blocks parsed from real output", blocks.len());
}
