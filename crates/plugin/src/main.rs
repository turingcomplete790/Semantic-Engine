//! Stash plugin executable (raw interface): one invocation per process.
//!
//! `semantic-engine --versions` prints the local versions as JSON (used by packaging); Stash
//! always starts the plugin without arguments.

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--versions") {
        println!(
            "{}",
            serde_json::to_string_pretty(&engine::versions()).unwrap()
        );
        return;
    }
    let stdin = plugin::read_stdin();
    let mut logger = plugin::log::Logger::stderr();
    let output = plugin::run(&stdin, &mut logger);
    println!("{output}");
}
