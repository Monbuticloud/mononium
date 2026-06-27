//! RPC configuration and error codes.

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

/// Default JSON-RPC WebSocket port.
pub const DEFAULT_RPC_PORT: u16 = 9944;

/// Default REST API port.
pub const DEFAULT_REST_PORT: u16 = 9933;

/// Default maximum concurrent RPC connections.
pub const DEFAULT_MAX_CONNECTIONS: u32 = 100;

// ---------------------------------------------------------------------------
// RpcConfig
// ---------------------------------------------------------------------------

/// Combined RPC + REST server configuration.
#[derive(Debug, Clone)]
pub struct RpcConfig {
    /// JSON-RPC WebSocket listen port.
    pub rpc_port: u16,
    /// REST API listen port.
    pub rest_port: u16,
    /// Maximum concurrent connections across both servers.
    pub max_connections: u32,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            rpc_port: DEFAULT_RPC_PORT,
            rest_port: DEFAULT_REST_PORT,
            max_connections: DEFAULT_MAX_CONNECTIONS,
        }
    }
}

// ---------------------------------------------------------------------------
// Error codes (consistent across REST + JSON-RPC)
// ---------------------------------------------------------------------------

/// Standardised error codes shared by REST and JSON-RPC endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpcErrorCode {
    Success = 0,
    Internal = -1,
    InvalidParams = -2,
    TxValidation = -3,
    BlockNotFound = -4,
    TxNotFound = -5,
    AddressNotFound = -6,
    RateLimited = -7,
}

impl RpcErrorCode {
    /// Human-readable message for each code.
    #[must_use]
    pub const fn message(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Internal => "internal error",
            Self::InvalidParams => "invalid parameters",
            Self::TxValidation => "transaction validation failed",
            Self::BlockNotFound => "block not found",
            Self::TxNotFound => "transaction not found",
            Self::AddressNotFound => "address not found",
            Self::RateLimited => "rate limited",
        }
    }

    /// Numeric code value.
    #[must_use]
    pub const fn code(self) -> i32 {
        self as i32
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = RpcConfig::default();
        assert_eq!(cfg.rpc_port, 9944);
        assert_eq!(cfg.rest_port, 9933);
        assert_eq!(cfg.max_connections, 100);
    }

    #[test]
    fn test_custom_config() {
        let cfg = RpcConfig {
            rpc_port: 19559,
            rest_port: 18080,
            max_connections: 500,
        };
        assert_eq!(cfg.rpc_port, 19559);
        assert_eq!(cfg.rest_port, 18080);
        assert_eq!(cfg.max_connections, 500);
    }

    #[test]
    fn test_error_code_values() {
        assert_eq!(RpcErrorCode::Success.code(), 0);
        assert_eq!(RpcErrorCode::Internal.code(), -1);
        assert_eq!(RpcErrorCode::InvalidParams.code(), -2);
        assert_eq!(RpcErrorCode::TxValidation.code(), -3);
        assert_eq!(RpcErrorCode::BlockNotFound.code(), -4);
        assert_eq!(RpcErrorCode::TxNotFound.code(), -5);
        assert_eq!(RpcErrorCode::AddressNotFound.code(), -6);
        assert_eq!(RpcErrorCode::RateLimited.code(), -7);
    }

    #[test]
    fn test_error_code_messages() {
        assert_eq!(RpcErrorCode::Success.message(), "success");
        assert_eq!(RpcErrorCode::Internal.message(), "internal error");
        assert_eq!(RpcErrorCode::InvalidParams.message(), "invalid parameters");
        assert_eq!(
            RpcErrorCode::TxValidation.message(),
            "transaction validation failed"
        );
        assert_eq!(RpcErrorCode::BlockNotFound.message(), "block not found");
        assert_eq!(RpcErrorCode::TxNotFound.message(), "transaction not found");
        assert_eq!(RpcErrorCode::AddressNotFound.message(), "address not found");
        assert_eq!(RpcErrorCode::RateLimited.message(), "rate limited");
    }

    #[test]
    fn test_all_error_codes_unique() {
        use std::collections::HashSet;
        let codes: Vec<i32> = vec![
            RpcErrorCode::Success.code(),
            RpcErrorCode::Internal.code(),
            RpcErrorCode::InvalidParams.code(),
            RpcErrorCode::TxValidation.code(),
            RpcErrorCode::BlockNotFound.code(),
            RpcErrorCode::TxNotFound.code(),
            RpcErrorCode::AddressNotFound.code(),
            RpcErrorCode::RateLimited.code(),
        ];
        let unique: HashSet<_> = codes.iter().copied().collect();
        assert_eq!(codes.len(), unique.len());
    }
}
