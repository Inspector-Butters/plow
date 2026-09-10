pub(crate) fn shell_quote(value: &str) -> String {
    let mut quoted = String::from("'");
    for ch in value.chars() {
        match ch {
            '\'' => quoted.push_str("'\\''"),
            // Fish interprets backslashes even inside single quotes. Escape
            // them outside quotes so both Fish and POSIX shells preserve them.
            '\\' => quoted.push_str("'\\\\'"),
            _ => quoted.push(ch),
        }
    }
    quoted.push('\'');
    quoted
}
