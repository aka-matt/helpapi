use thiserror::Error;

/// Errors that can occur in the mock engine.
#[derive(Debug, Clone, PartialEq, Error, serde::Serialize, serde::Deserialize)]
pub enum EngineError {
    /// Configuration or rule compilation failed.
    #[error("compile error: {0}")]
    CompileError(String),

    /// Rule matching failed.
    #[error("match error: {0}")]
    MatchError(String),

    /// A transform operation failed.
    #[error("transform error: {0}")]
    TransformError(String),

    /// The request or response body type is not supported.
    #[error("unsupported body type: {0}")]
    UnsupportedBodyType(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_error_display() {
        let err = EngineError::CompileError("invalid regex".to_string());
        assert_eq!(err.to_string(), "compile error: invalid regex");

        let err = EngineError::MatchError("no matching route".to_string());
        assert_eq!(err.to_string(), "match error: no matching route");

        let err = EngineError::TransformError("JSON pointer not found".to_string());
        assert_eq!(err.to_string(), "transform error: JSON pointer not found");

        let err = EngineError::UnsupportedBodyType("video/mp4".to_string());
        assert_eq!(err.to_string(), "unsupported body type: video/mp4");
    }

    #[test]
    fn test_engine_error_serialization() {
        let err = EngineError::CompileError("missing required field".to_string());
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("CompileError"));
        assert!(json.contains("missing required field"));

        // Verify round-trip
        let parsed: EngineError = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, EngineError::CompileError(_)));
    }

    #[test]
    fn test_engine_error_all_variants_serialize() {
        let variants = [
            EngineError::CompileError("test".to_string()),
            EngineError::MatchError("test".to_string()),
            EngineError::TransformError("test".to_string()),
            EngineError::UnsupportedBodyType("test".to_string()),
        ];

        for err in variants {
            let json = serde_json::to_string(&err).unwrap();
            let parsed: EngineError = serde_json::from_str(&json).unwrap();
            assert_eq!(err, parsed);
        }
    }

    #[test]
    fn test_engine_error_debug() {
        let err = EngineError::MatchError("route not found".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("MatchError"));
        assert!(debug.contains("route not found"));
    }
}
