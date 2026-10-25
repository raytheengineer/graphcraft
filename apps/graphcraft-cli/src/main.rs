//! GraphCraft CLI — M1.
//!
//! Subcommands:
//!   graphcraft-cli commands         — list registered engine commands
//!   graphcraft-cli run <id>         — run one command on an empty session
//!   graphcraft-cli new <PATH>       — write an empty .graphcraft document
//!
//! Later milestones add perf, bench, mcp, run <FILE>.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [] => {
            print_usage();
            ExitCode::SUCCESS
        }
        [arg] if matches!(arg.as_str(), "--help" | "-h") => {
            print_usage();
            ExitCode::SUCCESS
        }
        [sub] if sub == "commands" => {
            let reg = graphcraft_engine::CommandRegistry::new();
            for id in reg.ids() {
                println!("{id}");
            }
            ExitCode::SUCCESS
        }
        [sub, id] if sub == "run" => {
            let reg = graphcraft_engine::CommandRegistry::new();
            let mut s = graphcraft_engine::Session::default();
            match reg.run(&mut s, id, &serde_json::Value::Null) {
                Ok(v) => {
                    println!("{v}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::from(1)
                }
            }
        }
        [sub, path] if sub == "new" => {
            let doc = graphcraft_format::Document::empty();
            match graphcraft_format::serialize(&doc) {
                Ok(bytes) => match std::fs::write(path, bytes) {
                    Ok(()) => {
                        println!("wrote {path}");
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("write failed: {e}");
                        ExitCode::from(1)
                    }
                },
                Err(e) => {
                    eprintln!("serialize failed: {e}");
                    ExitCode::from(1)
                }
            }
        }
        _ => {
            print_usage();
            ExitCode::from(2)
        }
    }
}

fn print_usage() {
    eprintln!(
        "graphcraft-cli (pre-alpha)\n\
         usage:\n  \
         graphcraft-cli commands\n  \
         graphcraft-cli run <command_id>\n  \
         graphcraft-cli new <path>"
    );
}
