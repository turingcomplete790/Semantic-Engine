//! Measurement harness for the Phase 1 spike (contracts/spike-bench.md).

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = spike::bench::main_with(
        &args,
        &|k| std::env::var(k).ok(),
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    );
    std::process::exit(code);
}
