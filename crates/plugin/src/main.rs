//! Stash plugin executable (raw interface): one invocation per process.

fn main() {
    let stdin = plugin::read_stdin();
    let mut logger = plugin::log::Logger::stderr();
    let output = plugin::run(&stdin, &mut logger);
    println!("{output}");
}
