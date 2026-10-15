use clap::Parser;
use semantic_vocabulary::{load, Error, CONTRACT_VERSION};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "vocab-check", about = "Check the semantic vocabulary files")]
struct Args {
    /// Directory holding the vocabulary files
    #[arg(long, default_value = "vocabulary")]
    dir: PathBuf,
    /// Output format: text or json
    #[arg(long, default_value = "text")]
    format: String,
    /// Regenerate TERMS.md instead of only checking it is current
    #[arg(long)]
    write_terms: bool,
    /// Write released/<version>.json (the release snapshot)
    #[arg(long)]
    write_snapshot: bool,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let mut voc = match load(&args.dir) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {e}");
            if args.format == "json" {
                println!(
                    "{}",
                    serde_json::json!({"contractVersion": CONTRACT_VERSION, "ok": false,
                        "error": e.to_string(), "violations": []})
                );
            }
            return ExitCode::from(match e {
                Error::Io { .. } | Error::Parse { .. } | Error::BlankNode { .. } => 2,
            });
        }
    };
    if args.write_terms {
        let md = voc.generate_terms_md();
        if let Err(e) = std::fs::write(args.dir.join("TERMS.md"), &md) {
            eprintln!("error: cannot write TERMS.md: {e}");
            return ExitCode::from(2);
        }
        voc.terms_md = Some(md);
    }
    if args.write_snapshot {
        let snap = voc.snapshot();
        let dir = args.dir.join("released");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(format!("{}.json", snap.version));
        if let Err(e) = std::fs::write(&path, snap.to_json()) {
            eprintln!("error: cannot write {}: {e}", path.display());
            return ExitCode::from(2);
        }
        println!("wrote {}", path.display());
        voc.snapshots.retain(|s| s.version != snap.version);
        voc.snapshots.push(snap);
    }
    let violations = voc.check();
    if args.format == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "contractVersion": CONTRACT_VERSION,
                "vocabularyVersion": voc.version().unwrap_or_default(),
                "ok": violations.is_empty(),
                "violations": violations,
            }))
            .unwrap()
        );
    } else {
        if let Some(n) = voc.release_notice() {
            println!("notice: {n}");
        }
        for x in &violations {
            let loc = match (&x.file, x.line) {
                (Some(f), Some(l)) => format!("{f}:{l}"),
                (Some(f), None) => f.clone(),
                _ => "-".into(),
            };
            println!("{} {} {}", x.rule, loc, x.message);
        }
        if violations.is_empty() {
            println!("all rules pass");
        }
    }
    if violations.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
