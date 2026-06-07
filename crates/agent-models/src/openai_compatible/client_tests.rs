use super::build_chat_completions_url;

#[test]
fn appends_suffix_to_plain_base_url() {
    assert_eq!(
        build_chat_completions_url("https://api.openai.com/v1"),
        "https://api.openai.com/v1/chat/completions"
    );
}

#[test]
fn appends_suffix_to_base_url_with_trailing_slash() {
    assert_eq!(
        build_chat_completions_url("https://api.openai.com/v1/"),
        "https://api.openai.com/v1/chat/completions"
    );
}

#[test]
fn does_not_duplicate_when_url_already_has_suffix() {
    assert_eq!(
        build_chat_completions_url(
            "https://idealab.alibaba-inc.com/api/openai/v1/chat/completions"
        ),
        "https://idealab.alibaba-inc.com/api/openai/v1/chat/completions"
    );
}

#[test]
fn does_not_duplicate_when_url_has_suffix_and_trailing_slash() {
    assert_eq!(
        build_chat_completions_url(
            "https://idealab.alibaba-inc.com/api/openai/v1/chat/completions/"
        ),
        "https://idealab.alibaba-inc.com/api/openai/v1/chat/completions"
    );
}

#[test]
fn handles_localhost_base_url() {
    assert_eq!(
        build_chat_completions_url("http://localhost:11434/v1"),
        "http://localhost:11434/v1/chat/completions"
    );
}
