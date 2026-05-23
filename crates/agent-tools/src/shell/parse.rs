//! Shell command tokenizer and program/args splitter.
//!
//! Handles single quotes, double quotes (with `\\`-escapes for `"`, `\\`,
//! `$`, `` ` ``), and backslash-escaped whitespace. Unclosed quotes are
//! preserved verbatim (the opening quote is re-attached to the trailing
//! token) so callers see the original input rather than a silent merge.

pub fn parse_command(command: &str) -> (String, Vec<String>) {
    let tokens = tokenize(command);
    match tokens.split_first() {
        Some((program, args)) => (program.clone(), args.to_vec()),
        None => (String::new(), Vec::new()),
    }
}

fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;

    while let Some(ch) = chars.next() {
        if in_single {
            if ch == '\'' {
                in_single = false;
            } else {
                current.push(ch);
            }
        } else if in_double {
            if ch == '"' {
                in_double = false;
            } else if ch == '\\' {
                if let Some(&next) = chars.peek() {
                    if matches!(next, '"' | '\\' | '$' | '`') {
                        current.push(next);
                        chars.next();
                    } else {
                        current.push('\\');
                        current.push(next);
                        chars.next();
                    }
                } else {
                    current.push('\\');
                }
            } else {
                current.push(ch);
            }
        } else if ch == '\'' {
            in_single = true;
        } else if ch == '"' {
            in_double = true;
        } else if ch == '\\' {
            if let Some(&next) = chars.peek() {
                current.push(next);
                chars.next();
            } else {
                current.push('\\');
            }
        } else if ch.is_whitespace() {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
        } else {
            current.push(ch);
        }
    }

    if in_double {
        // Unclosed double quote: keep opening quote, push accumulated content
        let mut fixed = String::from("\"");
        fixed.push_str(&current);
        tokens.push(fixed);
    } else if in_single {
        // Unclosed single quote: keep opening quote, push accumulated content
        let mut fixed = String::from("'");
        fixed.push_str(&current);
        tokens.push(fixed);
    } else if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}
