#[cli::command]
fn testfn(x: usize, y: usize) {
    println!("x: {x}");
    println!("y: {y}");
}

#[derive(clap::Args)]
struct T {
    x: &str,
}

fn main() {
    let mut ctx = cli::Globals::default();
    let mut cli = cli::Cli::default();
    cli.register(testfn);
    cli.exec(std::env::args(), &mut ctx)
        .expect("Failed to exec");
    println!("Hello, world!");
}
