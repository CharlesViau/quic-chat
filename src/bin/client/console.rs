use std::{
    io::{self, BufRead, Read},
    thread,
};

use anyhow::Context;
use quic_chat::protocol::MAX_MESSAGE_BYTES;
use tokio::sync::mpsc::{self, Receiver, Sender};

const INPUT_QUEUE_CAPACITY: usize = 16;

pub(super) fn read_messages() -> anyhow::Result<Receiver<io::Result<String>>> {
    let (sender, receiver) = mpsc::channel(INPUT_QUEUE_CAPACITY);

    thread::Builder::new()
        .name("console-input".to_string())
        .spawn(move || {
            let stdin = io::stdin();
            forward_lines(&mut stdin.lock(), sender);
        })
        .context("Cannot start the console input thread")?;

    Ok(receiver)
}

fn forward_lines(input: &mut impl BufRead, sender: Sender<io::Result<String>>) {
    while !sender.is_closed() {
        match read_line(input) {
            Ok(InputLine::Message(message)) => {
                if sender.blocking_send(Ok(message)).is_err() {
                    break;
                }
            }
            Ok(InputLine::TooLong) => {
                eprintln!("Message skipped: maximum size is {MAX_MESSAGE_BYTES} UTF-8 bytes.");
            }
            Ok(InputLine::InvalidUtf8) => eprintln!("Message skipped: input is not valid UTF-8."),
            Ok(InputLine::End) => break,
            Err(error) => {
                let _ = sender.blocking_send(Err(error));
                break;
            }
        }
    }
}

#[derive(Debug, PartialEq)]
enum InputLine {
    Message(String),
    TooLong,
    InvalidUtf8,
    End,
}

fn read_line(input: &mut impl BufRead) -> io::Result<InputLine> {

    let limit = MAX_MESSAGE_BYTES + 2;
    let mut bytes = Vec::with_capacity(limit);
    (&mut *input)
        .take(limit as u64)
        .read_until(b'\n', &mut bytes)?;

    if bytes.is_empty() {
        return Ok(InputLine::End);
    }
    if bytes.len() == limit && !bytes.ends_with(b"\n") {
        input.skip_until(b'\n')?;
    }

    Ok(decode_line(bytes))
}

fn decode_line(mut bytes: Vec<u8>) -> InputLine {
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
        if bytes.last() == Some(&b'\r') {
            bytes.pop();
        }
    }
    if bytes.len() > MAX_MESSAGE_BYTES {
        return InputLine::TooLong;
    }

    match String::from_utf8(bytes) {
        Ok(message) => InputLine::Message(message),
        Err(_) => InputLine::InvalidUtf8,
    }
}

#[cfg(test)]
#[path = "console_tests.rs"]
mod tests;
