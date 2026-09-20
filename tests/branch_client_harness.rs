use aien_inference_abi::{ModelConfig, NativeTransformerBackend, TransformerWeights};

#[test]
fn test_openclaw_branch_client_2_8_32_fork_lifecycle() {
    let config = ModelConfig {
        num_layers: 2,
        num_heads: 4,
        num_kv_heads: 2,
        head_dim: 16,
        hidden_dim: 64,
        intermediate_dim: 128,
        vocab_size: 256,
        block_size: 16,
        ..Default::default()
    };

    let weights = TransformerWeights::reference_test_weights(&config);
    let mut runtime = NativeTransformerBackend::with_paged_kv(weights, 3000, 16).unwrap();

    // 1. Parent context with rich 64-token prompt (4 blocks)
    let prompt: Vec<u32> = (10..74).collect();
    let parent = runtime.create_context(&prompt).unwrap();

    // Loop through 2, 8, 32 branch scaling tiers
    for &branch_count in &[2, 8, 32] {
        let mut branches = Vec::new();
        for _ in 0..branch_count {
            let b = runtime.fork_context(parent).unwrap();
            branches.push(b);
        }

        // Before decode: all branches share 4 prefix blocks
        for &b in &branches {
            let r = runtime.get_usage_receipt(b).unwrap();
            assert_eq!(r.shared_pages, 4);
            assert_eq!(r.private_pages, 0);
            assert_eq!(r.cow_faults, 0);
            assert_eq!(r.logical_pages, 4);
            assert!(r.bytes_saved_vs_full_copy > 0);
        }

        // Each branch appends unique suffix tokens and generates
        for &b in &branches {
            let (tok, _) = runtime.decode_branch_step(b).unwrap();
            assert!(tok > 0);

            let r = runtime.get_usage_receipt(b).unwrap();
            assert_eq!(r.shared_pages, 4);
            assert_eq!(r.private_pages, 1);
            assert_eq!(r.logical_pages, 5);
        }

        // Release all branches in this tier
        for b in branches {
            runtime.release_branch(b).unwrap();
        }

        // Verify parent context remains healthy and uncorrupted
        let kv = runtime.kv_manager.as_ref().unwrap().read();
        assert_eq!(kv.active_sequence_count(), 1);
        assert_eq!(kv.metrics().physical_pages, 4);
        assert_eq!(kv.metrics().private_pages, 4);
    }
}
