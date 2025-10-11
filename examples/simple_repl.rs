// Simple REPL example that echoes what you type with "typed: " prefix

use editline::terminals::StdioTerminal;
use editline::LineEditor;

fn main() {
    println!("Simple REPL - Type something and press Enter");
    println!("Type 'exit' to quit");
    println!("Features: line editing, history (up/down), word navigation (Ctrl+arrows)");
    println!();

    let mut editor = LineEditor::new(1024, 50);
    let mut terminal = StdioTerminal::new();

    loop {
        print!("> ");
        std::io::Write::flush(&mut std::io::stdout()).unwrap();

        match editor.read_line(&mut terminal) {
            Ok(line) => {
                if line == "exit" {
                    println!("Goodbye!");
                    break;
                } else if !line.is_empty() {
                    println!("typed: {}", line);
                }
            }
            Err(e) => {
                eprintln!("\nError reading input: {}", e);
                break;
            }
        }
    }
}
