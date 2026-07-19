//! Build the runtime `Box<dyn Transform>` instances from typed `Transform` specs.

use crate::Transform;
use crate::transform::body::{ReplaceBody, SetStatus};
use crate::transform::headers::{RemoveHeader, SetHeader};
use crate::transform::json::{RemoveJsonPointer, SetJsonPointer};
use crate::transform::query::{RemoveQuery, SetQuery};
use crate::transform::spec::Transform as SpecTransform;

/// Builds a `Box<dyn Transform>` from a typed `Transform` spec.
pub fn transform_from_spec(spec: &SpecTransform) -> Box<dyn Transform> {
    match spec {
        SpecTransform::SetHeader { name, value } => {
            Box::new(SetHeader::new(name.clone(), value.clone()))
        }
        SpecTransform::RemoveHeader { name } => Box::new(RemoveHeader::new(name.clone())),
        SpecTransform::SetQuery { name, value } => {
            Box::new(SetQuery::new(name.clone(), value.clone()))
        }
        SpecTransform::RemoveQuery { name } => Box::new(RemoveQuery::new(name.clone())),
        SpecTransform::SetJsonPointer { path, value } => {
            Box::new(SetJsonPointer::new(path.clone(), value.clone()))
        }
        SpecTransform::RemoveJsonPointer { path } => Box::new(RemoveJsonPointer::new(path.clone())),
        SpecTransform::ReplaceBody { body } => Box::new(ReplaceBody::new(body.clone())),
        SpecTransform::SetStatus { status } => Box::new(SetStatus::new(*status)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_header_from_spec() {
        let spec = SpecTransform::SetHeader {
            name: "X-Foo".to_string(),
            value: "bar".to_string(),
        };
        let t = transform_from_spec(&spec);
        assert_eq!(t.name(), "SetHeader");
    }

    #[test]
    fn test_set_status_from_spec() {
        let spec = SpecTransform::SetStatus { status: 201 };
        let t = transform_from_spec(&spec);
        assert_eq!(t.name(), "SetStatus");
    }
}
