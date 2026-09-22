//! Sovereign sub-millisecond probe policy and tool call gating for AEGIS.
//!
//! Uses the native non-generative `aien-probe` engine to evaluate candidate actions
//! against security and containment probes before dispatching to the host.

use crate::security::SecurityError;
use aien_probe::{
    Choice, ChoiceOption, DeterministicReferenceBackend, HttpProbeBackend, Noul, Probe,
    ProbeBackend, ProbeEngine, ProbeSet,
};
use serde_json::json;

/// Evaluates candidate agent actions using sovereign non-generative probes.
pub struct ProbePolicyGuard<B: ProbeBackend + 'static> {
    engine: ProbeEngine<B>,
    min_safety_probability: f64,
}

impl ProbePolicyGuard<DeterministicReferenceBackend> {
    /// Constructs a policy guard using the in-process reference oracle.
    pub fn new_reference(min_safety_probability: f64) -> Self {
        Self {
            engine: ProbeEngine::new(DeterministicReferenceBackend::new()),
            min_safety_probability,
        }
    }
}

impl ProbePolicyGuard<HttpProbeBackend> {
    /// Constructs a policy guard targeting a local GB10 inference endpoint.
    pub fn new_http(endpoint: impl Into<String>, min_safety_probability: f64) -> Self {
        Self {
            engine: ProbeEngine::new(HttpProbeBackend::new(endpoint)),
            min_safety_probability,
        }
    }
}

impl<B: ProbeBackend + 'static> ProbePolicyGuard<B> {
    /// Constructs a policy guard over any custom probe engine backend.
    pub fn new(engine: ProbeEngine<B>, min_safety_probability: f64) -> Self {
        Self {
            engine,
            min_safety_probability,
        }
    }

    /// Evaluates a candidate tool action or shell command against safety and containment probes.
    pub async fn check_action(
        &self,
        action_name: &str,
        payload: &str,
    ) -> Result<bool, SecurityError> {
        let state = json!({
            "action": action_name,
            "payload": payload,
        });

        let probes = ProbeSet::new()
            .with(
                "safety",
                Probe::Noul(Noul::new(
                    "Is this agent action safe, non-destructive, and strictly within security bounds?",
                )),
            )
            .with(
                "boundary",
                Probe::Choice(Choice::new(
                    "Classify permission scope of the proposed action.",
                    [
                        ChoiceOption::new("contained").with_description("Operates inside permitted workspace"),
                        ChoiceOption::new("unauthorized_escalation").with_description("Escalates outside workspace"),
                    ],
                )),
            );

        let response =
            self.engine.evaluate(&state, &probes).await.map_err(|e| {
                SecurityError::AccessDenied(format!("Probe evaluation failed: {e}"))
            })?;

        let safety_prob = response.noul("safety").map(|a| a.noul).unwrap_or(0.0);
        let boundary = response
            .choice("boundary")
            .map(|a| a.choice.clone())
            .unwrap_or_else(|| "unauthorized_escalation".to_string());

        if safety_prob < self.min_safety_probability || boundary == "unauthorized_escalation" {
            return Err(SecurityError::AccessDenied(format!(
                "Action '{}' blocked by sovereign probe policy: safety {:.4} < {:.4}, scope '{}'",
                action_name, safety_prob, self.min_safety_probability, boundary
            )));
        }

        Ok(true)
    }
}
