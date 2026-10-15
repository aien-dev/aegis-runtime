//! Small fixed-size vector math used by the gateway and the CLI: cosine
//! similarity, token entropy, token projection, temperature scaling and a
//! four-lane accumulator. Pure Rust, no external libraries.
//!
//! Cosine similarity, projection and temperature scaling return 0.0 when an
//! input is NaN or infinite; token entropy skips such entries.

pub struct VectorMath;

impl VectorMath {
    pub fn cosine_similarity_4d(a: [f32; 4], b: [f32; 4]) -> f32 {
        for i in 0..4 {
            if a[i].is_nan() || b[i].is_nan() || a[i].is_infinite() || b[i].is_infinite() {
                return 0.0;
            }
        }
        let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
        let norm_a = a[0] * a[0] + a[1] * a[1] + a[2] * a[2] + a[3] * a[3];
        let norm_b = b[0] * b[0] + b[1] * b[1] + b[2] * b[2] + b[3] * b[3];
        if norm_a <= 0.0 || norm_b <= 0.0 || norm_a.is_nan() || norm_b.is_nan() {
            0.0
        } else {
            let denom = norm_a.sqrt() * norm_b.sqrt();
            if denom <= 0.0 || denom.is_nan() {
                0.0
            } else {
                let res = dot / denom;
                if res.is_nan() || res.is_infinite() {
                    0.0
                } else {
                    res
                }
            }
        }
    }

    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        if a.len() == 4 {
            let a_arr = [a[0], a[1], a[2], a[3]];
            let b_arr = [b[0], b[1], b[2], b[3]];
            return Self::cosine_similarity_4d(a_arr, b_arr);
        }

        let mut dot = 0.0f32;
        let mut norm_a = 0.0f32;
        let mut norm_b = 0.0f32;

        for i in 0..a.len() {
            if a[i].is_nan() || b[i].is_nan() || a[i].is_infinite() || b[i].is_infinite() {
                return 0.0;
            }
            dot += a[i] * b[i];
            norm_a += a[i] * a[i];
            norm_b += b[i] * b[i];
        }

        if norm_a <= 0.0 || norm_b <= 0.0 || norm_a.is_nan() || norm_b.is_nan() {
            return 0.0;
        }

        let denom = norm_a.sqrt() * norm_b.sqrt();
        if denom <= 0.0 || denom.is_nan() {
            return 0.0;
        }

        let res = dot / denom;
        if res.is_nan() || res.is_infinite() {
            0.0
        } else {
            res
        }
    }

    pub fn accumulate(base_val: f32, scale: f32, steps: i32) -> f32 {
        if base_val.is_nan() || scale.is_nan() || steps <= 0 {
            return 0.0;
        }
        let mut acc = [base_val, base_val * 1.5, base_val * 2.0, base_val * 2.5];
        let step_vec = [scale, scale * 1.1, scale * 1.2, scale * 1.3];
        let add_vec = [0.01f32, 0.02, 0.03, 0.04];
        for _ in 0..steps {
            for i in 0..4 {
                acc[i] = acc[i] * step_vec[i] + add_vec[i];
            }
        }
        acc.iter().sum()
    }

    pub fn token_entropy(probs: [f32; 4]) -> f32 {
        let mut sum = 0.0f32;
        for p in probs {
            if p.is_nan() || p.is_infinite() {
                continue;
            }
            if p > 0.00001 {
                sum += -p * p.ln();
            }
        }
        sum
    }

    pub fn token_projection(tokens: [f32; 4], weights: [f32; 4], bias: f32) -> f32 {
        for i in 0..4 {
            if tokens[i].is_nan()
                || weights[i].is_nan()
                || tokens[i].is_infinite()
                || weights[i].is_infinite()
            {
                return 0.0;
            }
        }
        if bias.is_nan() || bias.is_infinite() {
            return 0.0;
        }
        tokens[0] * weights[0]
            + tokens[1] * weights[1]
            + tokens[2] * weights[2]
            + tokens[3] * weights[3]
            + bias
    }

    pub fn temperature_scale(logit: f32, temperature: f32) -> f32 {
        if logit.is_nan()
            || temperature.is_nan()
            || logit.is_infinite()
            || temperature.is_infinite()
        {
            return 0.0;
        }
        if temperature <= 0.0001 {
            logit
        } else {
            let res = logit / temperature;
            if res.is_nan() || res.is_infinite() {
                logit
            } else {
                res
            }
        }
    }

    pub fn l2_norm(vec: &[f32]) -> f32 {
        let sum: f32 = vec.iter().map(|v| v * v).sum();
        sum.sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let v1 = vec![1.0, 0.0, 0.0, 0.0];
        let v2 = vec![1.0, 0.0, 0.0, 0.0];
        let sim = VectorMath::cosine_similarity(&v1, &v2);
        assert!((sim - 1.0).abs() < 1e-4);

        let v3 = vec![0.0, 1.0, 0.0, 0.0];
        let orthogonal = VectorMath::cosine_similarity(&v1, &v3);
        assert!((orthogonal - 0.0).abs() < 1e-4);
    }

    #[test]
    fn test_token_projection() {
        let tokens = [1.0, 2.0, 3.0, 4.0];
        let weights = [0.5, 0.5, 0.5, 0.5];
        let bias = 1.0;
        let proj = VectorMath::token_projection(tokens, weights, bias);
        let expected = 1.0 * 0.5 + 2.0 * 0.5 + 3.0 * 0.5 + 4.0 * 0.5 + 1.0;
        assert!((proj - expected).abs() < 1e-4);
    }

    #[test]
    fn test_temperature_scale() {
        let scaled = VectorMath::temperature_scale(2.0, 0.5);
        assert!((scaled - 4.0).abs() < 1e-4);
    }

    #[test]
    fn test_zero_vectors() {
        let zero = [0.0f32, 0.0, 0.0, 0.0];
        let non_zero = [1.0f32, 2.0, 3.0, 4.0];
        let sim1 = VectorMath::cosine_similarity_4d(zero, non_zero);
        assert_eq!(sim1, 0.0);

        let sim2 = VectorMath::cosine_similarity_4d(zero, zero);
        assert_eq!(sim2, 0.0);

        let zero_slice = vec![0.0f32; 8];
        let non_zero_slice = vec![1.0f32; 8];
        assert_eq!(
            VectorMath::cosine_similarity(&zero_slice, &non_zero_slice),
            0.0
        );
    }

    #[test]
    fn test_identical_and_opposite_vectors() {
        let v1 = [3.0f32, -4.0, 5.0, -1.0];
        let sim_ident = VectorMath::cosine_similarity_4d(v1, v1);
        assert!((sim_ident - 1.0).abs() < 1e-4);

        let v_opp = [-3.0f32, 4.0, -5.0, 1.0];
        let sim_opp = VectorMath::cosine_similarity_4d(v1, v_opp);
        assert!((sim_opp - (-1.0)).abs() < 1e-4);
    }

    #[test]
    fn test_orthogonal_vectors() {
        let v1 = [1.0f32, 0.0, 0.0, 0.0];
        let v2 = [0.0f32, 1.0, 0.0, 0.0];
        let sim = VectorMath::cosine_similarity_4d(v1, v2);
        assert_eq!(sim, 0.0);

        let v3 = [0.0f32, 0.0, 1.0, 0.0];
        let v4 = [0.0f32, 0.0, 0.0, 1.0];
        assert_eq!(VectorMath::cosine_similarity_4d(v3, v4), 0.0);
    }

    #[test]
    fn test_nan_and_inf_handling() {
        let nan_vec = [f32::NAN, 1.0, 2.0, 3.0];
        let normal_vec = [1.0f32, 2.0, 3.0, 4.0];
        assert_eq!(VectorMath::cosine_similarity_4d(nan_vec, normal_vec), 0.0);

        let inf_vec = [f32::INFINITY, 1.0, 2.0, 3.0];
        assert_eq!(VectorMath::cosine_similarity_4d(inf_vec, normal_vec), 0.0);

        let neg_inf_vec = [f32::NEG_INFINITY, 1.0, 2.0, 3.0];
        assert_eq!(
            VectorMath::cosine_similarity_4d(neg_inf_vec, normal_vec),
            0.0
        );

        // Token projection with NaN/Inf
        assert_eq!(VectorMath::token_projection(nan_vec, normal_vec, 1.0), 0.0);
        assert_eq!(
            VectorMath::token_projection(normal_vec, normal_vec, f32::NAN),
            0.0
        );

        // Temperature scale with NaN/Inf
        assert_eq!(VectorMath::temperature_scale(f32::NAN, 1.0), 0.0);
        assert_eq!(VectorMath::temperature_scale(2.0, f32::NAN), 0.0);
        assert_eq!(VectorMath::temperature_scale(2.0, f32::INFINITY), 0.0);
    }

    #[test]
    fn test_temperature_scale_edge_cases() {
        // Zero or near-zero temperature should return unscaled logit
        assert_eq!(VectorMath::temperature_scale(5.0, 0.0), 5.0);
        assert_eq!(VectorMath::temperature_scale(5.0, 0.00005), 5.0);

        // Normal scaling
        let scaled = VectorMath::temperature_scale(10.0, 2.0);
        assert!((scaled - 5.0).abs() < 1e-4);

        let low_temp = VectorMath::temperature_scale(10.0, 0.5);
        assert!((low_temp - 20.0).abs() < 1e-4);
    }

    #[test]
    fn test_token_projection_and_entropy() {
        let tokens = [2.0f32, 4.0, 6.0, 8.0];
        let weights = [0.1f32, 0.2, 0.3, 0.4];
        let bias = 0.5f32;
        let proj = VectorMath::token_projection(tokens, weights, bias);
        let expected = 2.0 * 0.1 + 4.0 * 0.2 + 6.0 * 0.3 + 8.0 * 0.4 + 0.5;
        assert!((proj - expected).abs() < 1e-4);

        let entropy = VectorMath::token_entropy([0.5, 0.5, 0.0, 0.0]);
        assert!((entropy - std::f32::consts::LN_2).abs() < 1e-4);
        assert!(entropy > 0.0);

        let zero_entropy = VectorMath::token_entropy([1.0, 0.0, 0.0, 0.0]);
        assert!((zero_entropy - 0.0).abs() < 1e-4);

        let uniform4_entropy = VectorMath::token_entropy([0.25, 0.25, 0.25, 0.25]);
        assert!((uniform4_entropy - (2.0 * std::f32::consts::LN_2)).abs() < 1e-4);

        // Mismatched slice length
        assert_eq!(VectorMath::cosine_similarity(&[1.0, 2.0], &[1.0]), 0.0);
        assert_eq!(VectorMath::cosine_similarity(&[], &[]), 0.0);
    }

    fn f64_4(value: &serde_json::Value, key: &str) -> [f64; 4] {
        let items = value[key].as_array().expect(key);
        [
            items[0].as_f64().unwrap(),
            items[1].as_f64().unwrap(),
            items[2].as_f64().unwrap(),
            items[3].as_f64().unwrap(),
        ]
    }

    fn f32_4(value: [f64; 4]) -> [f32; 4] {
        [
            value[0] as f32,
            value[1] as f32,
            value[2] as f32,
            value[3] as f32,
        ]
    }

    fn shannon_entropy_ref(probs: [f64; 4]) -> f64 {
        let mut h = 0.0;
        for p in probs {
            if p > 1e-5 && p.is_finite() {
                h -= p * p.ln();
            }
        }
        h
    }

    fn cosine_similarity_4d_ref(a: [f64; 4], b: [f64; 4]) -> f64 {
        for x in a.into_iter().chain(b) {
            if !x.is_finite() {
                return 0.0;
            }
        }
        let dot = a.iter().zip(b).map(|(x, y)| x * y).sum::<f64>();
        let norm_a = a.iter().map(|x| x * x).sum::<f64>();
        let norm_b = b.iter().map(|x| x * x).sum::<f64>();
        if norm_a <= 0.0 || norm_b <= 0.0 {
            return 0.0;
        }
        let denom = norm_a.sqrt() * norm_b.sqrt();
        if denom <= 0.0 {
            return 0.0;
        }
        let res = dot / denom;
        if res.is_finite() {
            res
        } else {
            0.0
        }
    }

    fn token_projection_ref(tokens: [f64; 4], weights: [f64; 4], bias: f64) -> f64 {
        for x in tokens.into_iter().chain(weights) {
            if !x.is_finite() {
                return 0.0;
            }
        }
        if !bias.is_finite() {
            return 0.0;
        }
        tokens.iter().zip(weights).map(|(t, w)| t * w).sum::<f64>() + bias
    }

    fn temperature_scale_ref(logit: f64, temp: f64) -> f64 {
        if !logit.is_finite() || !temp.is_finite() {
            return 0.0;
        }
        if temp <= 1e-4 {
            return logit;
        }
        let res = logit / temp;
        if res.is_finite() {
            res
        } else {
            logit
        }
    }

    fn accumulate_ref(base_val: f64, scale: f64, steps: i32) -> f64 {
        if !base_val.is_finite() || !scale.is_finite() || steps <= 0 {
            return 0.0;
        }
        let mut acc = [base_val, base_val * 1.5, base_val * 2.0, base_val * 2.5];
        let step_vec = [scale, scale * 1.1, scale * 1.2, scale * 1.3];
        let add_vec = [0.01, 0.02, 0.03, 0.04];
        for _ in 0..steps {
            for i in 0..4 {
                acc[i] = acc[i] * step_vec[i] + add_vec[i];
            }
        }
        acc.iter().sum()
    }

    fn assert_close(kind: &str, name: &serde_json::Value, got: f64, expected: f64, tol: f64) {
        assert!(
            (got - expected).abs() < tol,
            "{kind} parity failure for {name}: got {got} expected {expected}"
        );
    }

    #[test]
    fn test_analytical_parity() {
        let data = std::fs::read_to_string("tests/fixtures/simd_math_vectors.json")
            .expect("committed golden vectors");
        let v: serde_json::Value = serde_json::from_str(&data).expect("valid JSON fixture");

        for case in v["entropy"].as_array().expect("entropy cases") {
            let probs = f64_4(case, "probs");
            let golden = case["expected"].as_f64().unwrap();
            let analytical = shannon_entropy_ref(probs);
            assert_close(
                "entropy analytical",
                &case["name"],
                analytical,
                golden,
                1e-12,
            );
            assert_close(
                "entropy",
                &case["name"],
                f64::from(VectorMath::token_entropy(f32_4(probs))),
                golden,
                1e-4,
            );
        }

        for case in v["cosine"].as_array().expect("cosine cases") {
            let a = f64_4(case, "a");
            let b = f64_4(case, "b");
            let golden = case["expected"].as_f64().unwrap();
            assert_close(
                "cosine analytical",
                &case["name"],
                cosine_similarity_4d_ref(a, b),
                golden,
                1e-12,
            );
            assert_close(
                "cosine",
                &case["name"],
                f64::from(VectorMath::cosine_similarity_4d(f32_4(a), f32_4(b))),
                golden,
                1e-4,
            );
        }

        for case in v["projection"].as_array().expect("projection cases") {
            let tokens = f64_4(case, "tokens");
            let weights = f64_4(case, "weights");
            let bias = case["bias"].as_f64().unwrap();
            let golden = case["expected"].as_f64().unwrap();
            assert_close(
                "projection analytical",
                &case["name"],
                token_projection_ref(tokens, weights, bias),
                golden,
                1e-12,
            );
            assert_close(
                "projection",
                &case["name"],
                f64::from(VectorMath::token_projection(
                    f32_4(tokens),
                    f32_4(weights),
                    bias as f32,
                )),
                golden,
                1e-4,
            );
        }

        for case in v["temperature"].as_array().expect("temperature cases") {
            let logit = case["logit"].as_f64().unwrap();
            let temp = case["temp"].as_f64().unwrap();
            let golden = case["expected"].as_f64().unwrap();
            assert_close(
                "temperature analytical",
                &case["name"],
                temperature_scale_ref(logit, temp),
                golden,
                1e-12,
            );
            assert_close(
                "temperature",
                &case["name"],
                f64::from(VectorMath::temperature_scale(logit as f32, temp as f32)),
                golden,
                1e-4,
            );
        }

        for case in v["accumulate"].as_array().expect("accumulate cases") {
            let base_val = case["base_val"].as_f64().unwrap();
            let scale = case["scale"].as_f64().unwrap();
            let steps = case["steps"].as_i64().unwrap() as i32;
            let golden = case["expected"].as_f64().unwrap();
            assert_close(
                "accumulate analytical",
                &case["name"],
                accumulate_ref(base_val, scale, steps),
                golden,
                1e-9,
            );
            assert_close(
                "accumulate",
                &case["name"],
                f64::from(VectorMath::accumulate(base_val as f32, scale as f32, steps)),
                golden,
                1e-4,
            );
        }
    }
}
