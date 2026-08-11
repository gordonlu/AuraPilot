use std::fmt;

// The first suite exercises only deterministic and simulated boundaries. Keep
// the remaining evidence labels available so later real-provider and recovery
// cases cannot be reported as ordinary unit coverage.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvidenceLevel {
    DeterministicProtocol,
    SimulatedAdapter,
    RealProvider,
    Recovery,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompatibilityOutcome {
    Passed,
    Failed,
    Unsupported,
    TimedOut,
    InfrastructureError,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CompatibilityResult {
    pub case_id: &'static str,
    pub provider: &'static str,
    pub evidence_level: EvidenceLevel,
    pub outcome: CompatibilityOutcome,
    pub failures: Vec<String>,
}

pub struct CompatibilityCase {
    result: CompatibilityResult,
}

impl CompatibilityCase {
    pub fn new(
        case_id: &'static str,
        provider: &'static str,
        evidence_level: EvidenceLevel,
    ) -> Self {
        Self {
            result: CompatibilityResult {
                case_id,
                provider,
                evidence_level,
                outcome: CompatibilityOutcome::Passed,
                failures: Vec::new(),
            },
        }
    }

    pub fn required(&mut self, condition: bool, description: impl Into<String>) {
        if !condition {
            self.result
                .failures
                .push(format!("required behavior missing: {}", description.into()));
        }
    }

    pub fn forbidden(&mut self, occurred: bool, description: impl Into<String>) {
        if occurred {
            self.result.failures.push(format!(
                "forbidden behavior observed: {}",
                description.into()
            ));
        }
    }

    pub fn finish(mut self) -> CompatibilityResult {
        if !self.result.failures.is_empty() {
            self.result.outcome = CompatibilityOutcome::Failed;
        }
        self.result
    }
}

pub fn assert_compatible(result: CompatibilityResult) {
    assert_eq!(result.outcome, CompatibilityOutcome::Passed, "{result}");
}

impl fmt::Display for CompatibilityResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "compatibility case `{}` for `{}` ({:?}) returned {:?}",
            self.case_id, self.provider, self.evidence_level, self.outcome
        )?;
        for failure in &self.failures {
            write!(formatter, "\n- {failure}")?;
        }
        Ok(())
    }
}

#[test]
fn compatibility_result_reports_required_and_forbidden_failures_separately() {
    let mut case = CompatibilityCase::new(
        "self-test",
        "test-provider",
        EvidenceLevel::DeterministicProtocol,
    );
    case.required(false, "process launched");
    case.forbidden(true, "task state changed by Push");
    let result = case.finish();

    assert_eq!(result.outcome, CompatibilityOutcome::Failed);
    assert_eq!(result.failures.len(), 2);
    assert!(result.failures[0].starts_with("required behavior missing:"));
    assert!(result.failures[1].starts_with("forbidden behavior observed:"));
}
