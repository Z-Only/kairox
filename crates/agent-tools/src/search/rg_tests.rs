use super::*;

// ── parse_rg_json_output ───────────────────────────────────────────────

#[test]
fn parse_rg_json_match_line() {
    let json = r#"{"type":"match","data":{"path":{"text":"src/main.rs"},"line_number":10,"lines":{"text":"fn main() {\n"},"submatches":[{"start":3,"end":8,"match":{"text":"main"}}]}}"#;
    let raw = json.as_bytes();
    let results = parse_rg_json_output(raw, 100).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].file_path, "src/main.rs");
    assert_eq!(results[0].line_number, 10);
    assert_eq!(results[0].line_content, "fn main() {\n");
    assert_eq!(results[0].match_start, 3);
    assert_eq!(results[0].match_end, 8);
}

#[test]
fn parse_rg_json_skips_non_match_lines() {
    let input = r#"{"type":"summary","data":{"elapsed":0.01}}
{"type":"match","data":{"path":{"text":"foo.rs"},"line_number":1,"lines":{"text":"hello\n"},"submatches":[{"start":0,"end":5,"match":{"text":"hello"}}]}}"#;
    let results = parse_rg_json_output(input.as_bytes(), 100).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].file_path, "foo.rs");
}

#[test]
fn parse_rg_json_respects_max_results() {
    let line1 = r#"{"type":"match","data":{"path":{"text":"a.rs"},"line_number":1,"lines":{"text":"a\n"},"submatches":[{"start":0,"end":1,"match":{"text":"a"}}]}}"#;
    let line2 = r#"{"type":"match","data":{"path":{"text":"b.rs"},"line_number":2,"lines":{"text":"b\n"},"submatches":[{"start":0,"end":1,"match":{"text":"b"}}]}}"#;
    let input = format!("{}\n{}", line1, line2);
    let results = parse_rg_json_output(input.as_bytes(), 1).unwrap();
    assert_eq!(results.len(), 1);
}

#[test]
fn parse_rg_json_empty_input() {
    let results = parse_rg_json_output(b"", 100).unwrap();
    assert!(results.is_empty());
}

#[test]
fn parse_rg_json_malformed_lines_skipped() {
    let input = b"not json at all\n{\"type\":\"match\",\"data\":{\"path\":{\"text\":\"a.rs\"},\"line_number\":1,\"lines\":{\"text\":\"line\\n\"},\"submatches\":[{\"start\":0,\"end\":4,\"match\":{\"text\":\"line\"}}]}}\n";
    let results = parse_rg_json_output(input, 100).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].file_path, "a.rs");
}

#[test]
fn parse_rg_json_missing_submatches() {
    let input = b"{\"type\":\"match\",\"data\":{\"path\":{\"text\":\"a.rs\"},\"line_number\":1,\"lines\":{\"text\":\"line\\n\"}}}\n";
    let results = parse_rg_json_output(input, 100).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].file_path, "a.rs");
    assert_eq!(results[0].match_start, 0);
    assert_eq!(results[0].match_end, 0);
}

#[test]
fn parse_rg_json_with_empty_lines() {
    let input = b"\n\n{\"type\":\"match\",\"data\":{\"path\":{\"text\":\"a.rs\"},\"line_number\":1,\"lines\":{\"text\":\"x\\n\"},\"submatches\":[{\"start\":0,\"end\":1,\"match\":{\"text\":\"x\"}}]}}\n\n\n";
    let results = parse_rg_json_output(input, 100).unwrap();
    assert_eq!(results.len(), 1);
}

#[test]
fn parse_rg_json_multiple_submatches_uses_first() {
    let input = b"{\"type\":\"match\",\"data\":{\"path\":{\"text\":\"a.rs\"},\"line_number\":1,\"lines\":{\"text\":\"foo bar baz\\n\"},\"submatches\":[{\"start\":0,\"end\":3,\"match\":{\"text\":\"foo\"}},{\"start\":4,\"end\":7,\"match\":{\"text\":\"bar\"}}]}}\n";
    let results = parse_rg_json_output(input, 100).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].match_start, 0);
    assert_eq!(results[0].match_end, 3);
}
