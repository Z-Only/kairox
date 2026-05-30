use super::*;

#[test]
fn display_roundtrip() {
    for p in [
        ApprovalPolicy::Never,
        ApprovalPolicy::OnRequest,
        ApprovalPolicy::Always,
    ] {
        assert_eq!(p.to_string().parse::<ApprovalPolicy>().unwrap(), p);
    }
}

#[test]
fn fromstr_aliases() {
    assert_eq!(
        "OnRequest".parse::<ApprovalPolicy>().unwrap(),
        ApprovalPolicy::OnRequest
    );
    assert_eq!(
        "on-request".parse::<ApprovalPolicy>().unwrap(),
        ApprovalPolicy::OnRequest
    );
}

#[test]
fn fromstr_invalid() {
    assert!("bogus".parse::<ApprovalPolicy>().is_err());
}

#[test]
fn serde_snake_case() {
    let s = serde_json::to_string(&ApprovalPolicy::OnRequest).unwrap();
    assert_eq!(s, "\"on_request\"");
    let back: ApprovalPolicy = serde_json::from_str(&s).unwrap();
    assert_eq!(back, ApprovalPolicy::OnRequest);
}

#[test]
fn default_is_on_request() {
    assert_eq!(ApprovalPolicy::default(), ApprovalPolicy::OnRequest);
}
