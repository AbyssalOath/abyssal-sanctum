//! Minimal terminal styling. Colour is used only when standard output is a
//! terminal and `NO_COLOR` is not set.

use std::io::{IsTerminal, Write as _};

use crate::catalog::Risk;

fn enabled() -> bool {
    std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal()
}

fn paint(code: &str, text: &str) -> String {
    if enabled() {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_owned()
    }
}

pub(crate) fn bold(text: &str) -> String {
    paint("1", text)
}

pub(crate) fn dim(text: &str) -> String {
    paint("2", text)
}

pub(crate) fn accent(text: &str) -> String {
    paint("1;38;2;63;169;245", text)
}

pub(crate) fn good(text: &str) -> String {
    paint("32", text)
}

pub(crate) fn caution(text: &str) -> String {
    paint("33", text)
}

pub(crate) fn bad(text: &str) -> String {
    paint("1;31", text)
}

/// The risk label, coloured green, yellow or red.
pub(crate) fn risk(risk: Risk) -> String {
    match risk {
        Risk::ReadOnly => good(risk.label()),
        Risk::Modifies => caution(risk.label()),
        Risk::Destructive => bad(risk.label()),
    }
}

/// Write to standard output. If the reader has gone away (for example
/// `sanctum tools | head`), exit quietly instead of panicking.
pub(crate) fn write_out(args: std::fmt::Arguments<'_>) {
    let mut stdout = std::io::stdout().lock();
    if let Err(e) = stdout.write_fmt(args).and_then(|()| stdout.flush()) {
        if e.kind() == std::io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }
        eprintln!("sanctum: cannot write output: {e}");
        std::process::exit(1);
    }
}

/// Like `print!`, but exits quietly on a closed pipe.
macro_rules! out {
    ($($arg:tt)*) => { $crate::term::write_out(format_args!($($arg)*)) };
}

/// Like `println!`, but exits quietly on a closed pipe.
macro_rules! outln {
    () => { $crate::term::write_out(format_args!("\n")) };
    ($($arg:tt)*) => { $crate::term::write_out(format_args!("{}\n", format_args!($($arg)*))) };
}

pub(crate) use {out, outln};
