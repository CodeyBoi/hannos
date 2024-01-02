use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use pc_keyboard::DecodedKey;
use thiserror_no_std::Error;

use crate::{print, println};

pub struct Shell {
    buffer: Vec<char>,
    cursor_pos: usize,
    command_history: Vec<String>,
    command_history_index: usize,
}

#[derive(Error, Debug)]
pub enum ShellError {
    #[error("command not found: {0}")]
    CommandNotFound(String),
}

impl Shell {
    pub fn new() -> Self {
        let shell = Self {
            buffer: Vec::new(),
            cursor_pos: 0,
            command_history: Vec::new(),
            command_history_index: 0,
        };
        shell.render_input_line();
        shell
    }

    pub fn handle_keypress(&mut self, key: DecodedKey) {
        use pc_keyboard::KeyCode as KC;
        match key {
            DecodedKey::Unicode(c) => self.process_unicode(c),
            DecodedKey::RawKey(key) => match key {
                KC::ArrowUp => {
                    if self.command_history_index < self.command_history.len() {
                        self.command_history_index += 1;
                        self.replace_buffer_with_past_command();
                    }
                }
                KC::ArrowDown => {
                    if self.command_history_index > 0 {
                        self.command_history_index -= 1;
                        self.replace_buffer_with_past_command();
                    }
                }
                KC::ArrowLeft => self.cursor_pos = self.cursor_pos.checked_sub(1).unwrap_or(0),
                KC::ArrowRight => self.cursor_pos = self.buffer.len().min(self.cursor_pos + 1),
                _ => {}
            },
        }
        self.render_input_line();
    }

    fn render_input_line(&self) {
        print!("\r> {} ", self.buffer.iter().collect::<String>());
    }

    fn replace_buffer_with_past_command(&mut self) {
        print!("\r> {}\r", " ".repeat(self.buffer.len())); // clear input line
        self.buffer = if self.command_history_index == 0 {
            Vec::new()
        } else {
            self.command_history
                .get(self.command_history.len() - self.command_history_index)
                .unwrap()
                .chars()
                .collect()
        };
        self.cursor_pos = self.buffer.len();
    }

    fn process_unicode(&mut self, c: char) {
        match c {
            '\n' => self.process_buffer(),
            // match backspace
            '\u{8}' => {
                if self.cursor_pos == 0 {
                    return;
                }

                self.cursor_pos -= 1;
                if self.cursor_pos == self.buffer.len() {
                    self.buffer.pop();
                } else {
                    self.buffer.remove(self.cursor_pos);
                }
            }
            _ => {
                if self.cursor_pos == self.buffer.len() {
                    self.buffer.push(c);
                } else {
                    self.buffer.insert(self.cursor_pos, c);
                }
                self.cursor_pos += 1;
            }
        }
    }

    fn process_buffer(&mut self) {
        println!();
        let command = self.buffer.iter().collect::<String>();
        if command.is_empty() {
            return;
        }
        self.command_history.push(command.clone());
        let mut parts = command.split_whitespace();
        let command = parts.next().unwrap_or("");
        let args = parts.collect::<Vec<_>>();
        match Self::run_command(command, &args) {
            Ok(()) => {}
            Err(err) => println!("{}", err),
        }
        self.buffer.clear();
        self.cursor_pos = 0;
    }

    fn run_command(command: &str, args: &[&str]) -> Result<(), ShellError> {
        match command {
            "echo" => {
                println!("{}", args.join(" "));
            }
            "help" => {
                println!("Available commands:");
                println!("  echo");
                println!("  help");
                println!("  clear");
            }
            "clear" => {
                for _ in 0..100 {
                    println!();
                }
            }
            _ => return Err(ShellError::CommandNotFound(command.to_string())),
        }
        Ok(())
    }
}
