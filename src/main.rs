use clap::Parser;

use codex_threads::cli::Cli;

fn main() {
    let cli = Cli::parse();
    let emit_json = cli.json;

    match codex_threads::run(cli) {
        Ok(rendered) => {
            if emit_json {
                println!("{}", rendered.json);
            } else {
                println!("{}", rendered.text);
            }
        }
        Err(error) => {
            if emit_json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": false,
                        "error": error.to_string()
                    })
                );
            } else {
                eprintln!("错误: {}", error);
            }
            std::process::exit(1);
        }
    }
}
