//! xmlwf — well-formedness checker CLI.
//!
//! Modelled after libexpat's `xmlwf`. Reads an XML file from a path argument
//! and exits 0 if it is well-formed, non-zero otherwise. Errors go to stderr.
//!
//! Usage:  xmlwf <path>

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("usage: {} <xml-file>", args[0]);
        return ExitCode::from(2);
    }
    let path = &args[1];
    let src = match std::fs::read_to_string(path) {
        Ok(s)  => s,
        Err(e) => {
            eprintln!("{}: {}", path, e);
            return ExitCode::from(2);
        }
    };

    let mut parser = expat_rs::Parser::new(&src);
    loop {
        match parser.next_event() {
            Ok(Some(_)) => continue,
            Ok(None)    => return ExitCode::from(0),
            Err(e)      => {
                eprintln!("{}: {}", path, e);
                return ExitCode::from(1);
            }
        }
    }
}
