use super::file_uri_from_path;

#[test]
fn file_uri_from_path_percent_encodes_spaces() {
    assert_eq!(
        file_uri_from_path("/tmp/kairox project"),
        "file:///tmp/kairox%20project"
    );
}
