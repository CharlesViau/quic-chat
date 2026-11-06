use std::io::{self, Cursor};

use tokio::sync::mpsc;

use super::{InputLine, MAX_MESSAGE_BYTES, forward_lines, read_line};

#[test]
fn input_preserves_spaces_and_only_removes_line_endings() -> io::Result<()> {
    let mut input = Cursor::new(b"  Hello  \r\n\ninside\rcarriage\nlast\r");

    assert_eq!(
        read_line(&mut input)?,
        InputLine::Message("  Hello  ".to_string())
    );
    assert_eq!(read_line(&mut input)?, InputLine::Message(String::new()));
    assert_eq!(
        read_line(&mut input)?,
        InputLine::Message("inside\rcarriage".to_string())
    );
    assert_eq!(
        read_line(&mut input)?,
        InputLine::Message("last\r".to_string())
    );
    assert_eq!(read_line(&mut input)?, InputLine::End);
    Ok(())
}

#[test]
fn a_full_size_unicode_message_with_crlf_is_accepted() -> io::Result<()> {
    let message = "é".repeat(MAX_MESSAGE_BYTES / 2);
    assert_eq!(message.len(), MAX_MESSAGE_BYTES);
    let mut input = Cursor::new(format!("{message}\r\n"));

    assert_eq!(read_line(&mut input)?, InputLine::Message(message));
    assert_eq!(read_line(&mut input)?, InputLine::End);
    Ok(())
}

#[test]
fn an_oversized_line_is_discarded_before_the_next_message() -> io::Result<()> {
    let oversized = "x".repeat(MAX_MESSAGE_BYTES * 8);
    let mut input = Cursor::new(format!("{oversized}\nHello\n"));

    assert_eq!(read_line(&mut input)?, InputLine::TooLong);
    assert_eq!(
        read_line(&mut input)?,
        InputLine::Message("Hello".to_string())
    );
    assert_eq!(read_line(&mut input)?, InputLine::End);
    Ok(())
}

#[test]
fn the_limit_counts_utf8_bytes_instead_of_characters() -> io::Result<()> {
    let oversized = "é".repeat(MAX_MESSAGE_BYTES / 2 + 1);
    let mut input = Cursor::new(format!("{oversized}\nNext"));

    assert_eq!(read_line(&mut input)?, InputLine::TooLong);
    assert_eq!(
        read_line(&mut input)?,
        InputLine::Message("Next".to_string())
    );
    Ok(())
}

#[test]
fn invalid_utf8_does_not_consume_the_following_line() -> io::Result<()> {
    let mut input = Cursor::new(b"\xff\nHello\n");

    assert_eq!(read_line(&mut input)?, InputLine::InvalidUtf8);
    assert_eq!(
        read_line(&mut input)?,
        InputLine::Message("Hello".to_string())
    );
    Ok(())
}

#[test]
fn end_of_input_keeps_the_last_message_without_a_newline() -> io::Result<()> {
    let mut input = Cursor::new(b"Hello");

    assert_eq!(
        read_line(&mut input)?,
        InputLine::Message("Hello".to_string())
    );
    assert_eq!(read_line(&mut input)?, InputLine::End);
    Ok(())
}

#[test]
fn rejected_input_is_skipped_and_valid_messages_reach_the_receiver() -> io::Result<()> {
    let oversized = "x".repeat(MAX_MESSAGE_BYTES + 1);
    let mut input = Cursor::new(format!("First\n{oversized}\nLast"));
    let (sender, mut receiver) = mpsc::channel(4);

    forward_lines(&mut input, sender);

    assert_eq!(receiver.blocking_recv().expect("First message")?, "First");
    assert_eq!(receiver.blocking_recv().expect("Last message")?, "Last");
    assert!(receiver.blocking_recv().is_none());
    Ok(())
}
