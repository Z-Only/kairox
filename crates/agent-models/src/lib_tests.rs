use super::*;

#[test]
fn rate_limit_429_is_recoverable() {
    let err = ModelError::Api {
        status: 429,
        message: "rate limit exceeded".into(),
    };
    assert!(err.is_recoverable());
}

#[test]
fn server_error_500_is_recoverable() {
    let err = ModelError::Api {
        status: 500,
        message: "internal server error".into(),
    };
    assert!(err.is_recoverable());
}

#[test]
fn server_error_503_is_recoverable() {
    let err = ModelError::Api {
        status: 503,
        message: "service unavailable".into(),
    };
    assert!(err.is_recoverable());
}

#[test]
fn connection_error_is_recoverable() {
    let err = ModelError::Connection("connection refused".into());
    assert!(err.is_recoverable());
}

#[test]
fn overloaded_message_is_recoverable() {
    let err = ModelError::Api {
        status: 529,
        message: r#"{"error":{"message":"overloaded"}}"#.into(),
    };
    assert!(err.is_recoverable());
}

#[test]
fn overloaded_in_http_error_is_recoverable() {
    let err = ModelError::Http {
        status: 200,
        message: "server is overloaded".into(),
    };
    assert!(err.is_recoverable());
}

#[test]
fn bad_request_400_is_not_recoverable() {
    let err = ModelError::Api {
        status: 400,
        message: "bad request".into(),
    };
    assert!(!err.is_recoverable());
}

#[test]
fn authentication_401_is_not_recoverable() {
    let err = ModelError::Api {
        status: 401,
        message: "unauthorized".into(),
    };
    assert!(!err.is_recoverable());
}

#[test]
fn permission_403_is_not_recoverable() {
    let err = ModelError::Api {
        status: 403,
        message: "forbidden".into(),
    };
    assert!(!err.is_recoverable());
}

#[test]
fn unprocessable_422_is_not_recoverable() {
    let err = ModelError::Api {
        status: 422,
        message: "unprocessable entity".into(),
    };
    assert!(!err.is_recoverable());
}

#[test]
fn stream_parse_is_not_recoverable() {
    let err = ModelError::StreamParse("invalid json".into());
    assert!(!err.is_recoverable());
}

#[test]
fn request_error_is_not_recoverable() {
    let err = ModelError::Request("missing api key".into());
    assert!(!err.is_recoverable());
}

#[test]
fn http_status_returns_status_for_http_error() {
    let err = ModelError::Http {
        status: 502,
        message: "bad gateway".into(),
    };
    assert_eq!(err.http_status(), Some(502));
}

#[test]
fn http_status_returns_status_for_api_error() {
    let err = ModelError::Api {
        status: 429,
        message: "rate limited".into(),
    };
    assert_eq!(err.http_status(), Some(429));
}

#[test]
fn http_status_returns_none_for_connection_error() {
    let err = ModelError::Connection("timeout".into());
    assert_eq!(err.http_status(), None);
}

#[test]
fn http_status_returns_none_for_request_error() {
    let err = ModelError::Request("missing key".into());
    assert_eq!(err.http_status(), None);
}

#[test]
fn http_status_returns_none_for_stream_parse_error() {
    let err = ModelError::StreamParse("bad json".into());
    assert_eq!(err.http_status(), None);
}
