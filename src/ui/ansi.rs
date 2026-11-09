//! Discord ANSI code block formatting for Slint StyledText.

use i_slint_core::styled_text::{FormattedSpan, Style, StyledTextParagraph};

const COLOR_GRAY: u32 = 0xFF4F545C; // 30
const COLOR_RED: u32 = 0xFFED4245; // 31 - Discord Red
const COLOR_GREEN: u32 = 0xFF57F287; // 32 - Discord Green
const COLOR_YELLOW: u32 = 0xFFFEE75C; // 33 - Discord Yellow
const COLOR_BLURPLE: u32 = 0xFF5865F2; // 34 - Discord Blurple
const COLOR_PINK: u32 = 0xFFEB459E; // 35 - Discord Pink / Magenta
const COLOR_CYAN: u32 = 0xFF00B0F4; // 36 - Discord Cyan
const COLOR_WHITE: u32 = 0xFFFFFFFF; // 37 - Discord White

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct AnsiState {
    pub color: Option<u32>,
    pub bold: bool,
    pub underline: bool,
}

/// Checks whether `text` contains ANSI escape sequences.
pub fn has_ansi(text: &str) -> bool {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        if bytes[i] == 0x1b && i + 1 < len && bytes[i + 1] == b'[' {
            return true;
        }
        if bytes[i] == b'[' && is_likely_ansi(text, i) {
            return true;
        }
        i += 1;
    }
    false
}

fn is_likely_ansi(input: &str, at: usize) -> bool {
    let slice = &input[at + 1..];
    if let Some(m_pos) = slice.find('m') {
        m_pos > 0
            && m_pos <= 16
            && slice[..m_pos]
                .chars()
                .all(|c| c.is_ascii_digit() || c == ';')
    } else {
        false
    }
}

/// Parses text containing ANSI escape sequences into a native Slint `StyledText`
/// without relying on Markdown or HTML parsing.
pub fn parse_ansi(input: &str) -> slint::StyledText {
    let mut paragraphs = Vec::new();
    let mut state = AnsiState::default();

    for line in input.split('\n') {
        let (para_text, formatting) = parse_line(line, &mut state);
        paragraphs.push(StyledTextParagraph {
            text: para_text,
            formatting,
            links: Vec::new(),
        });
    }

    slint::StyledText::from_paragraphs(paragraphs)
}

fn parse_line(line: &str, state: &mut AnsiState) -> (String, Vec<FormattedSpan>) {
    let mut text = String::with_capacity(line.len());
    let mut formatting = Vec::new();

    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    let mut span_start = 0;

    let flush_span = |span_start: usize,
                      end: usize,
                      state: &AnsiState,
                      formatting: &mut Vec<FormattedSpan>| {
        if end > span_start {
            let range = span_start..end;
            if let Some(color) = state.color {
                formatting.push(FormattedSpan {
                    range: range.clone(),
                    style: Style::Color(color),
                });
            }
            if state.bold {
                formatting.push(FormattedSpan {
                    range: range.clone(),
                    style: Style::Strong,
                });
            }
            if state.underline {
                formatting.push(FormattedSpan {
                    range,
                    style: Style::Underline,
                });
            }
        }
    };

    while i < len {
        let is_esc = bytes[i] == 0x1b && i + 1 < len && bytes[i + 1] == b'[';
        let is_bracket = bytes[i] == b'[' && is_likely_ansi(line, i);

        if is_esc || is_bracket {
            let start = if is_esc { i + 2 } else { i + 1 };
            if let Some(m_pos) = line[start..].find('m') {
                if m_pos <= 16 {
                    let code_str = &line[start..start + m_pos];
                    if code_str.chars().all(|c| c.is_ascii_digit() || c == ';') {
                        // Flush active span before style change
                        flush_span(span_start, text.len(), state, &mut formatting);

                        apply_ansi_codes(code_str, state);

                        span_start = text.len();
                        i = start + m_pos + 1;
                        continue;
                    }
                }
            }
        }

        let ch = line[i..].chars().next().unwrap();
        text.push(ch);
        i += ch.len_utf8();
    }

    // Flush any remaining active span to the end of the line
    flush_span(span_start, text.len(), state, &mut formatting);

    (text, formatting)
}

fn apply_ansi_codes(codes: &str, state: &mut AnsiState) {
    let parts: Vec<u32> = codes.split(';').filter_map(|s| s.parse().ok()).collect();
    if parts.is_empty() {
        return;
    }

    let mut idx = 0;
    while idx < parts.len() {
        let code = parts[idx];
        match code {
            0 => {
                state.color = None;
                state.bold = false;
                state.underline = false;
            }
            1 => state.bold = true,
            4 => state.underline = true,
            22 => state.bold = false,
            24 => state.underline = false,
            30 => state.color = Some(COLOR_GRAY),
            31 => state.color = Some(COLOR_RED),
            32 => state.color = Some(COLOR_GREEN),
            33 => state.color = Some(COLOR_YELLOW),
            34 => state.color = Some(COLOR_BLURPLE),
            35 => state.color = Some(COLOR_PINK),
            36 => state.color = Some(COLOR_CYAN),
            37 => state.color = Some(COLOR_WHITE),
            39 => state.color = None,
            38 => {
                // Extended color: 38;2;r;g;b
                if idx + 4 < parts.len() && parts[idx + 1] == 2 {
                    let r = (parts[idx + 2] & 0xFF) as u32;
                    let g = (parts[idx + 3] & 0xFF) as u32;
                    let b = (parts[idx + 4] & 0xFF) as u32;
                    state.color = Some(0xFF000000 | (r << 16) | (g << 8) | b);
                    idx += 4;
                }
            }
            _ => {}
        }
        idx += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_discord_rules_message() {
        let input = "[1;31m╭───『 [1;31mПравила [0m [1;31m 』\n|   [31m1 не кикать сокланов\n|   2 не нападать на союзкланов\n|   3 визиторы не люди\n|   4 соблюдать правила\n|   5 решение проверки примерно\n|   6 приписки обязательны\n|   7 за злоупотребление чинов выеб [0m\n[1;31m╰──────────────────────────────────────────[0m";
        let styled = parse_ansi(input);
        assert_eq!(styled.paragraphs.len(), 9);
        assert_eq!(styled.paragraphs[0].text, "╭───『 Правила   』");
        assert_eq!(styled.paragraphs[1].text, "|   1 не кикать сокланов");
        assert_eq!(styled.paragraphs[2].text, "|   2 не нападать на союзкланов");
        assert_eq!(styled.paragraphs[7].text, "|   7 за злоупотребление чинов выеб ");
        assert_eq!(styled.paragraphs[8].text, "╰──────────────────────────────────────────");

        // Line 2 has inherited red color
        assert_eq!(styled.paragraphs[2].formatting[0].style, Style::Color(COLOR_RED));
        // Line 8 has red color
        assert_eq!(styled.paragraphs[8].formatting[0].style, Style::Color(COLOR_RED));
        assert_eq!(styled.paragraphs[8].formatting[1].style, Style::Strong);
    }
}
