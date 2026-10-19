//! Measurement-only Stash plugin `semantic-engine-spike` (contracts/spike-plugin.md): one
//! invocation per process, like the real plugin.

fn main() {
    let stdin = plugin::read_stdin();
    let mut logger = plugin::log::Logger::stderr();
    let output = spike::plugin_ops::run(&stdin, &mut logger);
    println!("{output}");
}
