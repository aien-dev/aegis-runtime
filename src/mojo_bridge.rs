use libloading::{Library, Symbol};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const DEFAULT_SO_PATH: &str =
    "/home/drakestapleton/workspace/openclaw-rs/mojo/libopenclaw_simd.so";

pub fn default_so_path() -> PathBuf {
    if let Ok(p) = std::env::var("OPENCLAW_SIMD_SO_PATH") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return pb;
        }
    }
    if let Some(p) = option_env!("OPENCLAW_SIMD_SO_BUILT") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return pb;
        }
    }
    let local = PathBuf::from("mojo/libopenclaw_simd.so");
    if local.exists() {
        return local;
    }
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let candidate = PathBuf::from(manifest).join("mojo/libopenclaw_simd.so");
        if candidate.exists() {
            return candidate;
        }
    }
    PathBuf::from(DEFAULT_SO_PATH)
}

type FnVersion = unsafe extern "C" fn() -> i32;
type FnCosineSim4d = unsafe extern "C" fn(f32, f32, f32, f32, f32, f32, f32, f32) -> f32;
type FnSimdAccumulate = unsafe extern "C" fn(f32, f32, i32) -> f32;
type FnTokenEntropy = unsafe extern "C" fn(f32, f32, f32, f32) -> f32;
type FnTokenProjection = unsafe extern "C" fn(f32, f32, f32, f32, f32, f32, f32, f32, f32) -> f32;
type FnTemperatureScale = unsafe extern "C" fn(f32, f32) -> f32;

pub struct MojoSimdBindings {
    _lib: &'static Library,
    pub version: FnVersion,
    pub cosine_similarity_4d: FnCosineSim4d,
    pub simd_accumulate: FnSimdAccumulate,
    pub token_entropy_simd: FnTokenEntropy,
    pub token_projection_simd: FnTokenProjection,
    pub temperature_scale_simd: FnTemperatureScale,
}

static BINDINGS: OnceLock<Result<MojoSimdBindings, String>> = OnceLock::new();

impl MojoSimdBindings {
    pub fn load_from<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let p = path.as_ref();
        if !p.exists() {
            return Err(format!("Mojo SIMD library not found at {:?}", p));
        }

        unsafe {
            let lib: &'static Library = Box::leak(Box::new(
                Library::new(p).map_err(|e| format!("Failed to dlopen {:?}: {}", p, e))?,
            ));

            let version: Symbol<FnVersion> = lib
                .get(b"openclaw_simd_version\0")
                .map_err(|e| format!("Missing symbol openclaw_simd_version: {}", e))?;
            let cosine_similarity_4d: Symbol<FnCosineSim4d> = lib
                .get(b"openclaw_cosine_similarity_4d\0")
                .map_err(|e| format!("Missing symbol openclaw_cosine_similarity_4d: {}", e))?;
            let simd_accumulate: Symbol<FnSimdAccumulate> = lib
                .get(b"openclaw_simd_accumulate\0")
                .map_err(|e| format!("Missing symbol openclaw_simd_accumulate: {}", e))?;
            let token_entropy_simd: Symbol<FnTokenEntropy> = lib
                .get(b"openclaw_token_entropy_simd\0")
                .map_err(|e| format!("Missing symbol openclaw_token_entropy_simd: {}", e))?;
            let token_projection_simd: Symbol<FnTokenProjection> = lib
                .get(b"openclaw_token_projection_simd\0")
                .map_err(|e| format!("Missing symbol openclaw_token_projection_simd: {}", e))?;
            let temperature_scale_simd: Symbol<FnTemperatureScale> = lib
                .get(b"openclaw_temperature_scale_simd\0")
                .map_err(|e| format!("Missing symbol openclaw_temperature_scale_simd: {}", e))?;

            Ok(Self {
                version: *version,
                cosine_similarity_4d: *cosine_similarity_4d,
                simd_accumulate: *simd_accumulate,
                token_entropy_simd: *token_entropy_simd,
                token_projection_simd: *token_projection_simd,
                temperature_scale_simd: *temperature_scale_simd,
                _lib: lib,
            })
        }
    }

    pub fn global() -> Result<&'static Self, String> {
        let res = BINDINGS.get_or_init(|| {
            let path = default_so_path();
            Self::load_from(path)
        });

        match res {
            Ok(ref b) => Ok(b),
            Err(ref e) => Err(e.clone()),
        }
    }
}

pub struct MojoSimdBridge;

impl MojoSimdBridge {
    pub fn is_mojo_accelerated() -> bool {
        MojoSimdBindings::global().is_ok()
    }

    pub fn version() -> i32 {
        if let Ok(b) = MojoSimdBindings::global() {
            unsafe { (b.version)() }
        } else {
            0
        }
    }

    pub fn cosine_similarity_4d_fallback(a: [f32; 4], b: [f32; 4]) -> f32 {
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

    pub fn cosine_similarity_fallback(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        if a.len() == 4 {
            let a_arr = [a[0], a[1], a[2], a[3]];
            let b_arr = [b[0], b[1], b[2], b[3]];
            return Self::cosine_similarity_4d_fallback(a_arr, b_arr);
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

    pub fn simd_accumulate_fallback(base_val: f32, scale: f32, steps: i32) -> f32 {
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

    pub fn token_entropy_fallback(probs: [f32; 4]) -> f32 {
        let mut sum = 0.0f32;
        for p in probs {
            if p.is_nan() || p.is_infinite() {
                continue;
            }
            if p > 0.00001 {
                sum += p * (p - 1.0);
            }
        }
        sum
    }

    pub fn token_projection_fallback(tokens: [f32; 4], weights: [f32; 4], bias: f32) -> f32 {
        for i in 0..4 {
            if tokens[i].is_nan() || weights[i].is_nan() || tokens[i].is_infinite() || weights[i].is_infinite() {
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

    pub fn temperature_scale_fallback(logit: f32, temperature: f32) -> f32 {
        if logit.is_nan() || temperature.is_nan() || logit.is_infinite() || temperature.is_infinite() {
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

    pub fn cosine_similarity_4d(a: [f32; 4], b: [f32; 4]) -> f32 {
        if let Ok(bindings) = MojoSimdBindings::global() {
            unsafe {
                (bindings.cosine_similarity_4d)(a[0], a[1], a[2], a[3], b[0], b[1], b[2], b[3])
            }
        } else {
            Self::cosine_similarity_4d_fallback(a, b)
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

        Self::cosine_similarity_fallback(a, b)
    }

    pub fn simd_accumulate(base_val: f32, scale: f32, steps: i32) -> f32 {
        if let Ok(bindings) = MojoSimdBindings::global() {
            unsafe { (bindings.simd_accumulate)(base_val, scale, steps) }
        } else {
            Self::simd_accumulate_fallback(base_val, scale, steps)
        }
    }

    pub fn token_entropy(probs: [f32; 4]) -> f32 {
        if let Ok(bindings) = MojoSimdBindings::global() {
            unsafe { (bindings.token_entropy_simd)(probs[0], probs[1], probs[2], probs[3]) }
        } else {
            Self::token_entropy_fallback(probs)
        }
    }

    pub fn token_projection(tokens: [f32; 4], weights: [f32; 4], bias: f32) -> f32 {
        if let Ok(bindings) = MojoSimdBindings::global() {
            unsafe {
                (bindings.token_projection_simd)(
                    tokens[0], tokens[1], tokens[2], tokens[3], weights[0], weights[1], weights[2],
                    weights[3], bias,
                )
            }
        } else {
            Self::token_projection_fallback(tokens, weights, bias)
        }
    }

    pub fn temperature_scale(logit: f32, temperature: f32) -> f32 {
        if let Ok(bindings) = MojoSimdBindings::global() {
            unsafe { (bindings.temperature_scale_simd)(logit, temperature) }
        } else {
            Self::temperature_scale_fallback(logit, temperature)
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
    fn test_mojo_simd_version_or_fallback() {
        let ver = MojoSimdBridge::version();
        assert!(ver == 101 || ver == 0);
    }

    #[test]
    fn test_cosine_similarity() {
        let v1 = vec![1.0, 0.0, 0.0, 0.0];
        let v2 = vec![1.0, 0.0, 0.0, 0.0];
        let sim = MojoSimdBridge::cosine_similarity(&v1, &v2);
        assert!((sim - 1.0).abs() < 1e-4);

        let v3 = vec![0.0, 1.0, 0.0, 0.0];
        let orthogonal = MojoSimdBridge::cosine_similarity(&v1, &v3);
        assert!((orthogonal - 0.0).abs() < 1e-4);
    }

    #[test]
    fn test_token_projection() {
        let tokens = [1.0, 2.0, 3.0, 4.0];
        let weights = [0.5, 0.5, 0.5, 0.5];
        let bias = 1.0;
        let proj = MojoSimdBridge::token_projection(tokens, weights, bias);
        let expected = 1.0 * 0.5 + 2.0 * 0.5 + 3.0 * 0.5 + 4.0 * 0.5 + 1.0;
        assert!((proj - expected).abs() < 1e-4);
    }

    #[test]
    fn test_temperature_scale() {
        let scaled = MojoSimdBridge::temperature_scale(2.0, 0.5);
        assert!((scaled - 4.0).abs() < 1e-4);
    }

    #[test]
    fn test_fallback_zero_vectors() {
        let zero = [0.0f32, 0.0, 0.0, 0.0];
        let non_zero = [1.0f32, 2.0, 3.0, 4.0];
        let sim1 = MojoSimdBridge::cosine_similarity_4d_fallback(zero, non_zero);
        assert_eq!(sim1, 0.0);

        let sim2 = MojoSimdBridge::cosine_similarity_4d_fallback(zero, zero);
        assert_eq!(sim2, 0.0);

        let zero_slice = vec![0.0f32; 8];
        let non_zero_slice = vec![1.0f32; 8];
        assert_eq!(MojoSimdBridge::cosine_similarity_fallback(&zero_slice, &non_zero_slice), 0.0);
    }

    #[test]
    fn test_fallback_identical_and_opposite_vectors() {
        let v1 = [3.0f32, -4.0, 5.0, -1.0];
        let sim_ident = MojoSimdBridge::cosine_similarity_4d_fallback(v1, v1);
        assert!((sim_ident - 1.0).abs() < 1e-4);

        let v_opp = [-3.0f32, 4.0, -5.0, 1.0];
        let sim_opp = MojoSimdBridge::cosine_similarity_4d_fallback(v1, v_opp);
        assert!((sim_opp - (-1.0)).abs() < 1e-4);
    }

    #[test]
    fn test_fallback_orthogonal_vectors() {
        let v1 = [1.0f32, 0.0, 0.0, 0.0];
        let v2 = [0.0f32, 1.0, 0.0, 0.0];
        let sim = MojoSimdBridge::cosine_similarity_4d_fallback(v1, v2);
        assert_eq!(sim, 0.0);

        let v3 = [0.0f32, 0.0, 1.0, 0.0];
        let v4 = [0.0f32, 0.0, 0.0, 1.0];
        assert_eq!(MojoSimdBridge::cosine_similarity_4d_fallback(v3, v4), 0.0);
    }

    #[test]
    fn test_fallback_nan_and_inf_handling() {
        let nan_vec = [f32::NAN, 1.0, 2.0, 3.0];
        let normal_vec = [1.0f32, 2.0, 3.0, 4.0];
        assert_eq!(MojoSimdBridge::cosine_similarity_4d_fallback(nan_vec, normal_vec), 0.0);

        let inf_vec = [f32::INFINITY, 1.0, 2.0, 3.0];
        assert_eq!(MojoSimdBridge::cosine_similarity_4d_fallback(inf_vec, normal_vec), 0.0);

        let neg_inf_vec = [f32::NEG_INFINITY, 1.0, 2.0, 3.0];
        assert_eq!(MojoSimdBridge::cosine_similarity_4d_fallback(neg_inf_vec, normal_vec), 0.0);

        // Token projection with NaN/Inf
        assert_eq!(MojoSimdBridge::token_projection_fallback(nan_vec, normal_vec, 1.0), 0.0);
        assert_eq!(MojoSimdBridge::token_projection_fallback(normal_vec, normal_vec, f32::NAN), 0.0);

        // Temperature scale with NaN/Inf
        assert_eq!(MojoSimdBridge::temperature_scale_fallback(f32::NAN, 1.0), 0.0);
        assert_eq!(MojoSimdBridge::temperature_scale_fallback(2.0, f32::NAN), 0.0);
        assert_eq!(MojoSimdBridge::temperature_scale_fallback(2.0, f32::INFINITY), 0.0);
    }

    #[test]
    fn test_fallback_temperature_scale_edge_cases() {
        // Zero or near-zero temperature should return unscaled logit
        assert_eq!(MojoSimdBridge::temperature_scale_fallback(5.0, 0.0), 5.0);
        assert_eq!(MojoSimdBridge::temperature_scale_fallback(5.0, 0.00005), 5.0);

        // Normal scaling
        let scaled = MojoSimdBridge::temperature_scale_fallback(10.0, 2.0);
        assert!((scaled - 5.0).abs() < 1e-4);

        let low_temp = MojoSimdBridge::temperature_scale_fallback(10.0, 0.5);
        assert!((low_temp - 20.0).abs() < 1e-4);
    }

    #[test]
    fn test_fallback_token_projection_and_entropy() {
        let tokens = [2.0f32, 4.0, 6.0, 8.0];
        let weights = [0.1f32, 0.2, 0.3, 0.4];
        let bias = 0.5f32;
        let proj = MojoSimdBridge::token_projection_fallback(tokens, weights, bias);
        let expected = 2.0 * 0.1 + 4.0 * 0.2 + 6.0 * 0.3 + 8.0 * 0.4 + 0.5;
        assert!((proj - expected).abs() < 1e-4);

        let entropy = MojoSimdBridge::token_entropy_fallback([0.5, 0.5, 0.0, 0.0]);
        assert!(entropy < 0.0);

        // Mismatched slice length
        assert_eq!(MojoSimdBridge::cosine_similarity_fallback(&[1.0, 2.0], &[1.0]), 0.0);
        assert_eq!(MojoSimdBridge::cosine_similarity_fallback(&[], &[]), 0.0);
    }
}
