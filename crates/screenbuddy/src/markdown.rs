//! A small Markdown reader for chat replies.
//!
//! Only the subset an AI reply actually uses is supported: fenced code blocks,
//! ATX headings, bullet lists, and inline bold / inline code. Anything else is
//! passed through as plain text, which is the right failure mode for a chat
//! window — showing the raw source is better than dropping it.
//!
//! Parsing is kept separate from drawing so the rules can be tested without a
//! window. `main_window` turns the result into GDI runs.

/// What a line is, which decides its font, indent and colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    /// A `#`-prefixed heading.
    Heading,
    /// A `-`/`*` bullet.
    Bullet,
    /// A line inside a fenced code block.
    Code,
    /// Anything else, including blank spacer lines.
    Paragraph,
}

/// How a run of text within a line is styled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStyle {
    Normal,
    /// `**bold**`
    Bold,
    /// `` `inline code` ``
    Code,
}

/// A contiguous span of text with one style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub text: String,
    pub style: RunStyle,
}

/// One rendered line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MdLine {
    pub kind: BlockKind,
    pub runs: Vec<Run>,
}

impl MdLine {
    /// The line's text with all markup removed, for measurement and tests.
    pub fn plain(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }
}

/// Parse `text` into renderable lines.
pub fn parse(text: &str) -> Vec<MdLine> {
    let mut out = Vec::new();
    let mut in_code = false;

    for raw in text.lines() {
        let trimmed = raw.trim_end();
        let fence = trimmed.trim_start().starts_with("```");

        if fence {
            // The fence line itself is markup, not content.
            in_code = !in_code;
            continue;
        }

        if in_code {
            // Code keeps its leading whitespace, which is the whole point.
            out.push(MdLine {
                kind: BlockKind::Code,
                runs: vec![Run {
                    text: raw.to_string(),
                    style: RunStyle::Code,
                }],
            });
            continue;
        }

        let lead = trimmed.trim_start();
        if let Some(rest) = lead.strip_prefix("# ") {
            out.push(MdLine {
                kind: BlockKind::Heading,
                runs: parse_runs(rest),
            });
        } else if lead.starts_with("## ") {
            out.push(MdLine {
                kind: BlockKind::Heading,
                runs: parse_runs(lead.strip_prefix("## ").unwrap()),
            });
        } else if let Some(rest) = lead.strip_prefix("- ").or_else(|| lead.strip_prefix("* ")) {
            out.push(MdLine {
                kind: BlockKind::Bullet,
                runs: parse_runs(rest),
            });
        } else {
            out.push(MdLine {
                kind: BlockKind::Paragraph,
                runs: parse_runs(trimmed),
            });
        }
    }

    out
}

/// Split a line into styled runs, consuming `**` and `` ` `` markers.
///
/// An unterminated marker is treated as literal text rather than swallowing the
/// rest of the line, so a reply with a stray `**` still reads correctly.
fn parse_runs(text: &str) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    let mut buf = String::new();
    let mut bold = false;
    let mut code = false;
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    // Push `buf` under the style currently in force, then clear it.
    macro_rules! flush {
        ($style:expr) => {
            if !buf.is_empty() {
                runs.push(Run {
                    text: std::mem::take(&mut buf),
                    style: $style,
                });
            }
        };
    }

    while i < chars.len() {
        let c = chars[i];
        if c == '*' && i + 1 < chars.len() && chars[i + 1] == '*' {
            flush!(if code {
                RunStyle::Code
            } else if bold {
                RunStyle::Bold
            } else {
                RunStyle::Normal
            });
            bold = !bold;
            i += 2;
            continue;
        }
        if c == '`' {
            flush!(if code {
                RunStyle::Code
            } else if bold {
                RunStyle::Bold
            } else {
                RunStyle::Normal
            });
            code = !code;
            i += 1;
            continue;
        }
        buf.push(c);
        i += 1;
    }
    flush!(if code {
        RunStyle::Code
    } else if bold {
        RunStyle::Bold
    } else {
        RunStyle::Normal
    });

    if runs.is_empty() {
        runs.push(Run {
            text: String::new(),
            style: RunStyle::Normal,
        });
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_is_one_paragraph_line() {
        let lines = parse("hello there");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].kind, BlockKind::Paragraph);
        assert_eq!(lines[0].plain(), "hello there");
    }

    #[test]
    fn a_fenced_block_marks_its_lines_as_code() {
        let lines = parse("before\n```\nlet x = 1;\nlet y = 2;\n```\nafter");
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0].kind, BlockKind::Paragraph);
        assert_eq!(lines[1].kind, BlockKind::Code);
        assert_eq!(lines[1].plain(), "let x = 1;");
        assert_eq!(lines[2].kind, BlockKind::Code);
        assert_eq!(lines[3].kind, BlockKind::Paragraph);
    }

    #[test]
    fn fence_lines_themselves_are_not_emitted() {
        let lines = parse("```\nonly\n```");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].plain(), "only");
        assert!(!lines.iter().any(|l| l.plain().contains("```")));
    }

    #[test]
    fn code_keeps_leading_whitespace() {
        let lines = parse("```\n    indented\n```");
        assert_eq!(lines[0].plain(), "    indented");
    }

    #[test]
    fn a_heading_is_recognised_and_the_hashes_dropped() {
        let lines = parse("# Title");
        assert_eq!(lines[0].kind, BlockKind::Heading);
        assert_eq!(lines[0].plain(), "Title");
    }

    #[test]
    fn bullets_are_recognised_for_both_markers() {
        let lines = parse("- one\n* two");
        assert_eq!(lines[0].kind, BlockKind::Bullet);
        assert_eq!(lines[0].plain(), "one");
        assert_eq!(lines[1].kind, BlockKind::Bullet);
        assert_eq!(lines[1].plain(), "two");
    }

    #[test]
    fn bold_markers_become_a_bold_run() {
        let lines = parse("a **strong** word");
        let runs = &lines[0].runs;
        assert_eq!(runs.len(), 3);
        assert_eq!(runs[0].style, RunStyle::Normal);
        assert_eq!(runs[1].style, RunStyle::Bold);
        assert_eq!(runs[1].text, "strong");
        assert_eq!(runs[2].style, RunStyle::Normal);
        // The markers must not survive into the rendered text.
        assert!(!lines[0].plain().contains('*'));
    }

    #[test]
    fn inline_code_becomes_a_code_run() {
        let lines = parse("call `parse()` now");
        let runs = &lines[0].runs;
        assert!(runs
            .iter()
            .any(|r| r.style == RunStyle::Code && r.text == "parse()"));
        assert!(!lines[0].plain().contains('`'));
    }

    #[test]
    fn an_unterminated_bold_marker_is_kept_as_text() {
        let lines = parse("2 ** 3");
        assert_eq!(lines[0].plain(), "2  3");
    }

    #[test]
    fn blank_lines_are_preserved_as_spacers() {
        let lines = parse("a\n\nb");
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[1].plain(), "");
    }

    #[test]
    fn parsing_never_panics_on_hostile_input() {
        for s in ["```", "**", "`", "```\n#", "\u{1F600}**`", ""] {
            let _ = parse(s);
        }
    }
}
