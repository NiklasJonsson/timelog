#[cli::command]
fn testfn(x: usize) {
    println!("x: {x}");
}

fn main() {
    let mut ctx = cli::Context::default();
    let mut cli = cli::Cli::default();
    cli.register(testfn);
    cli.exec(std::env::args(), &mut ctx)
        .expect("Failed to exec");
    println!("Hello, world!");
}
