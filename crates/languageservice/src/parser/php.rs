//! PHP block masking for the HTML scanner.
//!
//! PHP blocks are opaque to the HTML language service. Their contents can look
//! like HTML, and the closing tag is not simply the first `?>`: strings,
//! heredocs and comments nest, and interpolated expressions switch back to
//! scripting mode. Mago's lexer finds the real block boundaries.
//!
//! Short open tags are disabled: `<?xml ... ?>` stays HTML, matching PHP's
//! default `short_open_tag=Off`.

use mago_database::file::FileId;
use mago_syntax::lexer::Lexer;
use mago_syntax::settings::LexerSettings;
use mago_syntax::token::TokenKind;
use mago_syntax_core::input::Input;

/// Byte ranges (`start..end`) of complete PHP blocks in `input`.
/// An unterminated block extends to the end of `input`, like PHP.
fn php_spans(input: &str) -> Vec<(usize, usize)> {
    let mut lexer = Lexer::new(
        Input::new(FileId::zero(), input.as_bytes()),
        LexerSettings {
            enable_short_tags: false,
        },
    );
    let mut spans = Vec::new();
    let mut open = None;

    loop {
        let token = match lexer.advance() {
            None => break,
            Some(Ok(token)) => token,
            // The lexer consumes the offending byte, so recovery keeps moving.
            Some(Err(_)) => continue,
        };
        match token.kind {
            TokenKind::OpenTag | TokenKind::EchoTag | TokenKind::ShortOpenTag => {
                open.get_or_insert(token.start.offset as usize);
            }
            TokenKind::CloseTag => {
                if let Some(start) = open.take() {
                    spans.push((start, token.start.offset as usize + token.value.len()));
                }
            }
            _ => {}
        }
    }

    if let Some(start) = open {
        spans.push((start, input.len()));
    }
    spans
}

/// Whether `input` contains a PHP opening tag. Keeps pure HTML (including
/// `<?xml ... ?>`) off the lexer path.
pub(crate) fn may_contain_php(input: &str) -> bool {
    let bytes = input.as_bytes();
    bytes.windows(3).any(|w| w == b"<?=")
        || bytes.windows(5).any(|w| {
            w[0] == b'<'
                && w[1] == b'?'
                && (w[2] | 0x20) == b'p'
                && (w[3] | 0x20) == b'h'
                && (w[4] | 0x20) == b'p'
        })
}

/// Replaces PHP blocks in `input` with spaces, preserving UTF-16 offsets and
/// newlines. Used to build [`TextDocument::get_scanner_text`](crate::text_document::TextDocument::get_scanner_text).
pub(crate) fn mask_text(input: &str) -> String {
    let mut spans = php_spans(input).into_iter().peekable();
    let mut masked = String::with_capacity(input.len());
    for (byte, ch) in input.char_indices() {
        while spans.peek().is_some_and(|&(_, end)| byte >= end) {
            spans.next();
        }
        let inside = spans
            .peek()
            .is_some_and(|&(start, end)| byte >= start && byte < end);
        if inside && ch != '\n' && ch != '\r' {
            // One space per UTF-16 code unit keeps offsets stable for astral
            // characters (two units, two spaces).
            for _ in 0..ch.len_utf16() {
                masked.push(' ');
            }
        } else {
            masked.push(ch);
        }
    }
    masked
}

/// Replaces PHP blocks in `input` with spaces in `units`.
///
/// `units` is the UTF-16 encoding of `input`. Newlines are preserved so line
/// offsets stay valid, and one space is written per UTF-16 code unit so column
/// offsets (including astral characters) stay valid too.
pub(crate) fn mask_php(input: &str, units: &mut [u16]) {
    if !may_contain_php(input) {
        return;
    }
    let mut spans = php_spans(input).into_iter().peekable();
    if spans.peek().is_none() {
        return;
    }

    let mut utf16 = 0;
    for (byte, ch) in input.char_indices() {
        while spans.peek().is_some_and(|&(_, end)| byte >= end) {
            spans.next();
        }
        let masked = spans
            .peek()
            .is_some_and(|&(start, end)| byte >= start && byte < end);
        let len = ch.len_utf16();
        if masked && ch != '\n' && ch != '\r' {
            units[utf16..utf16 + len].fill(b' ' as u16);
        }
        utf16 += len;
    }
}

#[cfg(test)]
mod tests {
    use super::{mask_php, mask_text, may_contain_php, php_spans};

    fn masked(input: &str) -> String {
        let mut units: Vec<u16> = input.encode_utf16().collect();
        mask_php(input, &mut units);
        String::from_utf16(&units).unwrap()
    }

    fn assert_spans(input: &str, expected: &[(usize, usize)]) {
        assert_eq!(php_spans(input), expected, "input: {input:?}");
    }

    #[test]
    fn close_tag_inside_string_does_not_close() {
        assert_spans(
            r#"<?php echo "?>"; ?><div>"#,
            &[(0, r#"<?php echo "?>"; ?>"#.len())],
        );
    }

    #[test]
    fn close_tag_inside_single_quoted_string_does_not_close() {
        assert_spans(
            "<?php echo '?>'; ?><div>",
            &[(0, "<?php echo '?>'; ?>".len())],
        );
    }

    #[test]
    fn hash_comment_close_tag_closes() {
        let input = "<?php # ?>\n<div>";
        let end = input.find("?>").unwrap() + 2;
        assert_spans(input, &[(0, end)]);
    }

    #[test]
    fn close_tag_inside_nested_interpolation_does_not_close() {
        assert_spans(
            r#"<?php echo "{$values["?>"]}"; ?><div>"#,
            &[(0, r#"<?php echo "{$values["?>"]}"; ?>"#.len())],
        );
    }

    #[test]
    fn line_comment_close_tag_closes() {
        let input = "<?php // ?>\n<div>";
        let end = input.find("?>").unwrap() + 2;
        assert_spans(input, &[(0, end)]);
    }

    #[test]
    fn block_comment_close_tag_does_not_close() {
        assert_spans(
            "<?php /* ?> */ echo 1; ?><div>",
            &[(0, "<?php /* ?> */ echo 1; ?>".len())],
        );
    }

    #[test]
    fn heredoc_close_tag_does_not_close() {
        let input = "<?php $x = <<<TXT\n?>\nTXT;\n?><div>";
        let end = input.rfind("?>").unwrap() + 2;
        assert_spans(input, &[(0, end)]);
    }

    #[test]
    fn unclosed_block_extends_to_eof() {
        assert_spans("<?php echo 1;", &[(0, 13)]);
    }

    #[test]
    fn multiple_blocks() {
        let input = "<?php echo 1; ?><b><?= $x ?></b>";
        assert_spans(input, &[(0, 16), (19, 19 + "<?= $x ?>".len())]);
    }

    #[test]
    fn xml_declaration_is_not_php() {
        assert_spans(r#"<?xml version="1.0"?><div>"#, &[]);
    }

    #[test]
    fn detects_php_openers_without_lexing_xml() {
        assert!(may_contain_php("<?php echo 1; ?>"));
        assert!(may_contain_php("<?PHP echo 1; ?>"));
        assert!(may_contain_php("<?= $x ?>"));
        assert!(!may_contain_php(r#"<?xml version="1.0"?>"#));
        assert!(!may_contain_php("<div>plain</div>"));
    }

    #[test]
    fn text_masking_matches_unit_masking() {
        let input = "<?php echo \"{$a[\"?>\"]}\"; ?>\n<div>😀</div>";
        assert_eq!(mask_text(input), masked(input));
        assert_eq!(
            mask_text(input).encode_utf16().count(),
            input.encode_utf16().count()
        );
    }

    #[test]
    fn masking_preserves_utf16_offsets_and_newlines() {
        let input = "<?php echo \"😀 ?>\"; ?>\n<div>😀</div>";
        let masked = masked(input);
        assert_eq!(masked.encode_utf16().count(), input.encode_utf16().count());
        assert_eq!(masked.matches('\n').count(), input.matches('\n').count());
        let utf16 = |s: &str, needle: &str| s[..s.find(needle).unwrap()].encode_utf16().count();
        assert_eq!(utf16(&masked, "<div>"), utf16(input, "<div>"));
    }
}
