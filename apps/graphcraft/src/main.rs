//! GraphCraft desktop app — M1 stub.
//!
//! Prints the registered engine commands and exits. The real app shell
//! lands in a later milestone.

fn main() {
    let reg = graphcraft_engine::CommandRegistry::new();
    println!("GraphCraft (pre-alpha). Registered commands:");
    for id in reg.ids() {
        println!("  {id}");
    }
}
