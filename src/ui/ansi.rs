//! Discord ANSI code block formatting for Slint StyledText.

/// Formats text containing ANSI escape sequences (e.g. from Discord ```ansi code blocks)
/// into Slint-compatible styled markdown with `<font color="...">`, `<b>`, and `<u>`.
pub fn format_ansi(input: &str) -> String {
    let mut output = String::with_capacity(input.len() * 2);
    let mut in_color = false;
    let mut in_bold = false;
    let mut in_underline = false;

    let mut i = 0;
    let bytes = input.as_bytes();
    let len = bytes.len();

    while i < len {
        let is_esc = bytes[i] == 0x1b && i + 1 < len && bytes[i + 1] == b'[';
        let is_bracket = bytes[i] == b'['
            && i + 1 < len
            && (bytes[i + 1].is_ascii_digit() || bytes[i + 1] == b';');

        if is_esc || (is_bracket && is_likely_ansi(input, i)) {
            let start = if is_esc { i + 2 } else { i + 1 };
            if let Some(m_pos) = input[start..].find('m') {
                if m_pos <= 10 {
                    let code_str = &input[start..start + m_pos];
                    if code_str.chars().all(|c| c.is_ascii_digit() || c == ';') {
                        apply_ansi_codes(
                            code_str,
                            &mut output,
                            &mut in_color,
                            &mut in_bold,
                            &mut in_underline,
                        );
                        i = start + m_pos + 1;
                        continue;
                    }
                }
            }
        }

        let ch = input[i..].chars().next().unwrap();
        match ch {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '*' => output.push_str("\\*"),
            '_' => output.push_str("\\_"),
            '~' => output.push_str("\\~"),
            '`' => output.push_str("\\`"),
            c => output.push(c),
        }
        i += ch.len_utf8();
    }

    if in_bold {
        output.push_str("</b>");
    }
    if in_underline {
        output.push_str("</u>");
    }
    if in_color {
        output.push_str("</font>");
    }

    output
}

fn is_likely_ansi(input: &str, at: usize) -> bool {
    let slice = &input[at + 1..];
    if let Some(m_pos) = slice.find('m') {
        m_pos <= 10 && slice[..m_pos].chars().all(|c| c.is_ascii_digit() || c == ';')
    } else {
        false
    }
}

fn apply_ansi_codes(
    codes: &str,
    output: &mut String,
    in_color: &mut bool,
    in_bold: &mut bool,
    in_underline: &mut bool,
) {
    let mut new_color = None;
    let mut new_bold = None;
    let mut new_underline = None;
    let mut reset = false;

    for part in codes.split(';') {
        let Ok(num) = part.parse::<u32>() else { continue };
        match num {
            0 => reset = true,
            1 => new_bold = Some(true),
            4 => new_underline = Some(true),
            // Standard colors
            30 => new_color = Some("#4f545c"),
            31 => new_color = Some("#ed4245"), // Discord Vibrant Red
            32 => new_color = Some("#57f287"), // Discord Vibrant Green
            33 => new_color = Some("#fee75c"), // Discord Vibrant Yellow
            34 => new_color = Some("#5865f2"), // Discord Blurple
            35 => new_color = Some("#eb459e"), // Discord Pink
            36 => new_color = Some("#00b0f4"), // Discord Cyan
            37 => new_color = Some("#ffffff"), // Discord White
            _ => {}
        }
    }

    if reset {
        if *in_bold {
            output.push_str("</b>");
            *in_bold = false;
        }
        if *in_underline {
            output.push_str("</u>");
            *in_underline = false;
        }
        if *in_color {
            output.push_str("</font>");
            *in_color = false;
        }
    }

    if let Some(color) = new_color {
        if *in_color {
            output.push_str("</font>");
        }
        output.push_str(&format!("<font color=\"{color}\">"));
        *in_color = true;
    }

    if let Some(true) = new_bold {
        if !*in_bold {
            output.push_str("<b>");
            *in_bold = true;
        }
    }

    if let Some(true) = new_underline {
        if !*in_underline {
            output.push_str("<u>");
            *in_underline = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_discord_ansi() {
        let input = "\x1b[1;31mПравила\x1b[0m";
        let formatted = format_ansi(input);
        assert_eq!(formatted, "<font color=\"#ed4245\"><b>Правила</b></font>");
    }

    #[test]
    fn parses_stripped_ansi() {
        let input = "[1;31mПравила[0m";
        let formatted = format_ansi(input);
        assert_eq!(formatted, "<font color=\"#ed4245\"><b>Правила</b></font>");
    }
}
