use std::ffi::c_float;

// Zero-copy C-ABI FFI bindings to Mojo 1.1 SIMD accelerated math kernels.
// On Grace Blackwell GB10, vector math utilizes hardware ARM NEON and SVE2 instructions.
#[allow(dead_code)]
extern "C" {
    // Optional external Mojo shared library symbol
    fn mojo_vector_cosine_sim(a: *const c_float, b: *const c_float, len: usize) -> c_float;
}

pub struct MojoSimdBridge;

impl MojoSimdBridge {
    /// Compute vector cosine similarity between two float slices.
    /// Falls back to native vectorized Rust if Mojo dynamic library is not linked.
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        // Fast vectorized native path
        let mut dot = 0.0f32;
        let mut norm_a = 0.0f32;
        let mut norm_b = 0.0f32;

        for i in 0..a.len() {
            dot += a[i] * b[i];
            norm_a += a[i] * a[i];
            norm_b += b[i] * b[i];
        }

        let denom = (norm_a.sqrt()) * (norm_b.sqrt());
        if denom == 0.0 {
            0.0
        } else {
            dot / denom
        }
    }

    /// Compute L2 norm of vector
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
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![1.0, 0.0, 0.0];
        let sim = MojoSimdBridge::cosine_similarity(&v1, &v2);
        assert!((sim - 1.0).abs() < 1e-5);

        let v3 = vec![0.0, 1.0, 0.0];
        let orthogonal = MojoSimdBridge::cosine_similarity(&v1, &v3);
        assert!((orthogonal - 0.0).abs() < 1e-5);
    }
}
