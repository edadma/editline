// Simple REPL example that echoes what you type with "typed: " prefix

use editline::terminals::StdioTerminal;
use editline::LineEditor;

fn main() {
    println!("Simple REPL - Type something and press Enter");
    println!("Type 'exit' or press Ctrl-D to quit");
    println!("Features: line editing, history (up/down), word navigation (Ctrl+arrows)");
    println!("Press Ctrl-C to cancel current line");
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
                // Handle Ctrl-C and Ctrl-D
                match e.kind() {
                    std::io::ErrorKind::UnexpectedEof => {
                        // Ctrl-D pressed - exit gracefully
                        println!("\nGoodbye!");
                        break;
                    }
                    std::io::ErrorKind::Interrupted => {
                        // Ctrl-C pressed - show message and continue
                        println!("\nInterrupted. Type 'exit' or press Ctrl-D to quit.");
                        continue;
                    }
                    _ => {
                        eprintln!("\nError reading input: {}", e);
                        break;
                    }
                }
            }
        }
    }
}
