//! laconic-check: deterministic candidates for Laconic's house style.
//!
//! `lint` reports candidates in the original prose of Markdown files; a match is a
//! reason to look, not proof of a defect. `preserve` compares an original and an
//! edit and reports numbers, code spans, URLs and negations the edit dropped.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::process::ExitCode;

mod lint;
mod preserve;

fn usage() -> ExitCode {
    eprintln!(
        "usage:\n  laconic-check lint [--warn] <file.md>...\n  laconic-check preserve <original> <edited>\n\n\
         lint exits 1 when it finds candidates, unless --warn is given.\n\
         preserve exits 1 when the edit dropped a protected item."
    );
    ExitCode::from(2)
}

fn read(path: &str) -> Result<String, ExitCode> {
    fs::read_to_string(path).map_err(|e| {
        eprintln!("{path}: {e}");
        ExitCode::from(2)
    })
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("lint") => {
            let warn = args.iter().any(|a| a == "--warn");
            let files: Vec<&String> = args[1..].iter().filter(|a| *a != "--warn").collect();
            if files.is_empty() {
                return usage();
            }
            let mut total = 0;
            for file in files {
                let text = match read(file) {
                    Ok(t) => t,
                    Err(code) => return code,
                };
                for f in lint::lint(&text) {
                    println!("{file}:{}:{}: {}: {}", f.line, f.column, f.rule, f.message);
                    total += 1;
                }
            }
            if total > 0 && !warn {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        Some("preserve") if args.len() == 3 => {
            let (original, edited) = match (read(&args[1]), read(&args[2])) {
                (Ok(o), Ok(e)) => (o, e),
                (Err(code), _) | (_, Err(code)) => return code,
            };
            let missing: BTreeMap<&str, Vec<String>> = preserve::missing(&original, &edited);
            if missing.is_empty() {
                println!("No protected items dropped.");
                return ExitCode::SUCCESS;
            }
            for (kind, items) in &missing {
                for item in items {
                    println!("dropped {kind}: {item}");
                }
            }
            ExitCode::from(1)
        }
        _ => usage(),
    }
}
