//! `Evidence`: what cancellAI knows and why (`docs/architecture/DOMAIN_MODEL.md` "Evidence").
//! Every safety-relevant classification references evidence IDs rather than asserting a
//! conclusion unbacked by an observation (C-06 "Evidence before action").

/// An opaque, stable reference to one [`Evidence`] record. Newtype rather than a bare
/// `String` so a caller cannot pass an arbitrary string where an evidence reference is
/// expected without at least going through this type's constructor.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct EvidenceId(pub String);

impl EvidenceId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl std::fmt::Display for EvidenceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// One concrete observation backing a classification or action (`docs/architecture/
/// DOMAIN_MODEL.md` "Evidence" - "provider metadata says project path X," "transcript
/// filename contains session UUID Y," "last observed mutation was 93 days ago," ...).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Evidence {
    pub id: EvidenceId,
    pub description: String,
}

impl Evidence {
    pub fn new(id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: EvidenceId::new(id),
            description: description.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_id_serializes_as_a_bare_string_not_a_wrapped_object() {
        let id = EvidenceId::new("evidence-0001");
        assert_eq!(
            serde_json::to_string(&id).expect("serializable"),
            "\"evidence-0001\""
        );
    }
}
