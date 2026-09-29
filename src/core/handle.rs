use uuid::Uuid;

use crate::core::error::CvxError;

/// The kind of object a handle refers to. Only parameters exist so far;
/// later specifications will add variables, expressions, and more.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleKind {
    Param,
    Var,
    Expr,
}

impl HandleKind {
    fn as_str(self) -> &'static str {
        match self {
            HandleKind::Param => "param",
            HandleKind::Var => "var",
            HandleKind::Expr => "expr",
        }
    }
}

/// Formats a handle as `cvx:<kind>:<uuid>`.
pub fn format_handle(kind: HandleKind, id: Uuid) -> String {
    format!("cvx:{}:{}", kind.as_str(), id)
}

/// Parses a handle of the form `cvx:<kind>:<uuid>`.
pub fn parse_handle(handle: &str) -> Result<(HandleKind, Uuid), CvxError> {
    let mut parts = handle.splitn(3, ':');
    let (prefix, kind, id) = match (parts.next(), parts.next(), parts.next()) {
        (Some(p), Some(k), Some(i)) => (p, k, i),
        _ => return Err(CvxError::InvalidHandle(handle.to_string())),
    };

    if prefix != "cvx" {
        return Err(CvxError::InvalidHandle(handle.to_string()));
    }

    let kind = match kind {
        "param" => HandleKind::Param,
        "var" => HandleKind::Var,
        "expr" => HandleKind::Expr,
        _ => return Err(CvxError::InvalidHandle(handle.to_string())),
    };

    let uuid = Uuid::parse_str(id).map_err(|_| CvxError::InvalidHandle(handle.to_string()))?;
    Ok((kind, uuid))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_a_parameter_handle() {
        let id = Uuid::new_v4();
        let handle = format_handle(HandleKind::Param, id);
        assert_eq!(parse_handle(&handle).unwrap(), (HandleKind::Param, id));
    }

    #[test]
    fn round_trips_a_variable_handle() {
        let id = Uuid::new_v4();
        let handle = format_handle(HandleKind::Var, id);
        assert_eq!(parse_handle(&handle).unwrap(), (HandleKind::Var, id));
    }

    #[test]
    fn round_trips_an_expression_handle() {
        let id = Uuid::new_v4();
        let handle = format_handle(HandleKind::Expr, id);
        assert_eq!(parse_handle(&handle).unwrap(), (HandleKind::Expr, id));
    }

    #[test]
    fn rejects_unknown_kind() {
        let handle = format!("cvx:unknown:{}", Uuid::new_v4());
        assert_eq!(parse_handle(&handle), Err(CvxError::InvalidHandle(handle)));
    }

    #[test]
    fn rejects_malformed_handle() {
        assert!(parse_handle("not-a-handle").is_err());
        assert!(parse_handle("cvx:param:not-a-uuid").is_err());
    }
}
