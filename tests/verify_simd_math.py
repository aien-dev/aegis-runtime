#!/usr/bin/env python3
"""
Python Mathematical Verification Oracle for OpenClaw SIMD & Mojo Kernels.
Validates all mathematical routines against analytical ground truth in Python.
Generates oracle test fixtures for Rust unit test consumption.
"""

import ctypes
import json
import math
import os
import sys

def shannon_entropy_ref(probs):
    h = 0.0
    for p in probs:
        if p > 1e-5 and math.isfinite(p):
            h -= p * math.log(p)
    return h

def cosine_similarity_4d_ref(a, b):
    for x in a + b:
        if math.isnan(x) or math.isinf(x):
            return 0.0
    dot = sum(x * y for x, y in zip(a, b))
    norm_a = sum(x * x for x in a)
    norm_b = sum(x * x for x in b)
    if norm_a <= 0.0 or norm_b <= 0.0:
        return 0.0
    denom = math.sqrt(norm_a) * math.sqrt(norm_b)
    if denom <= 0.0:
        return 0.0
    res = dot / denom
    if math.isnan(res) or math.isinf(res):
        return 0.0
    return res

def token_projection_ref(tokens, weights, bias):
    for x in tokens + weights:
        if math.isnan(x) or math.isinf(x):
            return 0.0
    if math.isnan(bias) or math.isinf(bias):
        return 0.0
    return sum(t * w for t, w in zip(tokens, weights)) + bias

def temperature_scale_ref(logit, temp):
    if math.isnan(logit) or math.isnan(temp) or math.isinf(logit) or math.isinf(temp):
        return 0.0
    if temp <= 1e-4:
        return logit
    res = logit / temp
    if math.isnan(res) or math.isinf(res):
        return logit
    return res

def simd_accumulate_ref(base_val, scale, steps):
    if math.isnan(base_val) or math.isnan(scale) or steps <= 0:
        return 0.0
    acc = [base_val, base_val * 1.5, base_val * 2.0, base_val * 2.5]
    step_vec = [scale, scale * 1.1, scale * 1.2, scale * 1.3]
    add_vec = [0.01, 0.02, 0.03, 0.04]
    for _ in range(steps):
        for i in range(4):
            acc[i] = acc[i] * step_vec[i] + add_vec[i]
    return sum(acc)

def main():
    print("==========================================================")
    print("OpenClaw SIMD Mathematical Verification Oracle (Python 3)")
    print("==========================================================")

    # 1. Define verification test sets
    entropy_cases = [
        {"name": "uniform_2", "probs": [0.5, 0.5, 0.0, 0.0]},
        {"name": "uniform_4", "probs": [0.25, 0.25, 0.25, 0.25]},
        {"name": "certainty", "probs": [1.0, 0.0, 0.0, 0.0]},
        {"name": "skewed", "probs": [0.7, 0.2, 0.08, 0.02]},
        {"name": "sparse_single", "probs": [0.0, 0.0, 1.0, 0.0]},
        {"name": "all_zero", "probs": [0.0, 0.0, 0.0, 0.0]},
    ]

    cosine_cases = [
        {"name": "identical", "a": [1.0, 2.0, 3.0, 4.0], "b": [1.0, 2.0, 3.0, 4.0]},
        {"name": "opposite", "a": [1.0, 0.0, 0.0, 0.0], "b": [-1.0, 0.0, 0.0, 0.0]},
        {"name": "orthogonal", "a": [1.0, 0.0, 0.0, 0.0], "b": [0.0, 1.0, 0.0, 0.0]},
        {"name": "zero_vector", "a": [0.0, 0.0, 0.0, 0.0], "b": [1.0, 2.0, 3.0, 4.0]},
        {"name": "mixed_positive_negative", "a": [3.0, -4.0, 5.0, -1.0], "b": [3.0, -4.0, 5.0, -1.0]},
    ]

    projection_cases = [
        {"name": "standard", "tokens": [1.0, 2.0, 3.0, 4.0], "weights": [0.5, 0.5, 0.5, 0.5], "bias": 1.0},
        {"name": "zero_bias", "tokens": [2.0, 4.0, 6.0, 8.0], "weights": [0.1, 0.2, 0.3, 0.4], "bias": 0.0},
        {"name": "negative_weights", "tokens": [1.0, -1.0, 2.0, -2.0], "weights": [0.5, -0.5, 0.5, -0.5], "bias": 2.5},
    ]

    temperature_cases = [
        {"name": "standard_half", "logit": 2.0, "temp": 0.5},
        {"name": "standard_double", "logit": 10.0, "temp": 2.0},
        {"name": "zero_temp_guard", "logit": 5.0, "temp": 0.0},
        {"name": "near_zero_temp_guard", "logit": 5.0, "temp": 0.00005},
    ]

    accumulate_cases = [
        {"name": "short_acc", "base_val": 1.0, "scale": 1.05, "steps": 5},
        {"name": "single_step", "base_val": 2.0, "scale": 1.1, "steps": 1},
    ]

    # Calculate expected outputs
    for c in entropy_cases:
        c["expected"] = shannon_entropy_ref(c["probs"])
        assert c["expected"] >= 0.0, f"Entropy must be non-negative: {c}"

    for c in cosine_cases:
        c["expected"] = cosine_similarity_4d_ref(c["a"], c["b"])

    for c in projection_cases:
        c["expected"] = token_projection_ref(c["tokens"], c["weights"], c["bias"])

    for c in temperature_cases:
        c["expected"] = temperature_scale_ref(c["logit"], c["temp"])

    for c in accumulate_cases:
        c["expected"] = simd_accumulate_ref(c["base_val"], c["scale"], c["steps"])

    # 2. Check against Mojo shared library if available
    base_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    so_path = os.path.join(base_dir, "mojo", "libopenclaw_simd.so")

    if os.path.exists(so_path):
        lib = ctypes.CDLL(so_path)
        lib.openclaw_simd_version.restype = ctypes.c_int32
        lib.openclaw_cosine_similarity_4d.restype = ctypes.c_float
        lib.openclaw_cosine_similarity_4d.argtypes = [ctypes.c_float]*8
        lib.openclaw_token_entropy_simd.restype = ctypes.c_float
        lib.openclaw_token_entropy_simd.argtypes = [ctypes.c_float]*4
        lib.openclaw_token_projection_simd.restype = ctypes.c_float
        lib.openclaw_token_projection_simd.argtypes = [ctypes.c_float]*9
        lib.openclaw_temperature_scale_simd.restype = ctypes.c_float
        lib.openclaw_temperature_scale_simd.argtypes = [ctypes.c_float]*2
        lib.openclaw_simd_accumulate.restype = ctypes.c_float
        lib.openclaw_simd_accumulate.argtypes = [ctypes.c_float, ctypes.c_float, ctypes.c_int32]

        print(f"Loaded Mojo SIMD library: {so_path}")
        print(f"SIMD Version: {lib.openclaw_simd_version()}")

        # Verify Entropy
        for c in entropy_cases:
            got = lib.openclaw_token_entropy_simd(*c["probs"])
            diff = abs(got - c["expected"])
            print(f"  [ENTROPY] {c['name']:<15} expected={c['expected']:.6f} got={got:.6f} diff={diff:.2e}")
            assert diff < 1e-4, f"Mojo entropy mismatch for {c['name']}: {got} vs {c['expected']}"

        # Verify Cosine
        for c in cosine_cases:
            got = lib.openclaw_cosine_similarity_4d(*(c["a"] + c["b"]))
            diff = abs(got - c["expected"])
            print(f"  [COSINE]  {c['name']:<15} expected={c['expected']:.6f} got={got:.6f} diff={diff:.2e}")
            assert diff < 1e-4, f"Mojo cosine mismatch for {c['name']}: {got} vs {c['expected']}"

        # Verify Projection
        for c in projection_cases:
            got = lib.openclaw_token_projection_simd(*(c["tokens"] + c["weights"] + [c["bias"]]))
            diff = abs(got - c["expected"])
            print(f"  [PROJ]    {c['name']:<15} expected={c['expected']:.6f} got={got:.6f} diff={diff:.2e}")
            assert diff < 1e-4, f"Mojo projection mismatch for {c['name']}: {got} vs {c['expected']}"

        # Verify Temperature
        for c in temperature_cases:
            got = lib.openclaw_temperature_scale_simd(c["logit"], c["temp"])
            diff = abs(got - c["expected"])
            print(f"  [TEMP]    {c['name']:<15} expected={c['expected']:.6f} got={got:.6f} diff={diff:.2e}")
            assert diff < 1e-4, f"Mojo temperature mismatch for {c['name']}: {got} vs {c['expected']}"

        # Verify Accumulate
        for c in accumulate_cases:
            got = lib.openclaw_simd_accumulate(c["base_val"], c["scale"], c["steps"])
            diff = abs(got - c["expected"])
            print(f"  [ACCUM]   {c['name']:<15} expected={c['expected']:.6f} got={got:.6f} diff={diff:.2e}")
            assert diff < 1e-4, f"Mojo accumulate mismatch for {c['name']}: {got} vs {c['expected']}"

        print("All Mojo SIMD routines matched Python analytical reference within 1e-4.")

    # 3. Export fixtures for Rust test suite
    fixtures_dir = os.path.join(base_dir, "tests", "fixtures")
    os.makedirs(fixtures_dir, exist_ok=True)
    out_file = os.path.join(fixtures_dir, "simd_math_vectors.json")

    payload = {
        "entropy": entropy_cases,
        "cosine": cosine_cases,
        "projection": projection_cases,
        "temperature": temperature_cases,
        "accumulate": accumulate_cases,
    }

    with open(out_file, "w") as f:
        json.dump(payload, f, indent=2)

    print(f"Exported analytical test fixtures to: {out_file}")
    print("Python Mathematical Verification: SUCCESS.")

if __name__ == "__main__":
    main()
