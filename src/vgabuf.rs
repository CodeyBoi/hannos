use core::{arch::asm, fmt, ptr::addr_of_mut};

use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::instructions::interrupts;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct VGAColor(u8);

impl VGAColor {
    pub fn new(fg: Color, bg: Color) -> VGAColor {
        VGAColor((bg as u8) << 4 | (fg as u8))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct VGABufferEntry {
    ascii_char: u8,
    color: VGAColor,
}

const BUF_ADDR: usize = 0xb8000;
const WIDTH: usize = 80;
const HEIGHT: usize = 25;

#[repr(transparent)]
struct VGABuffer {
    chars: [[VGABufferEntry; WIDTH]; HEIGHT],
}

pub struct VGAWriter {
    row: usize,
    col: usize,
    color: VGAColor,
    buffer: VGABuffer,
    output: &'static mut VGABuffer,
}

impl VGAWriter {
    pub fn new() -> VGAWriter {
        VGAWriter {
            row: HEIGHT - 1,
            col: 0,
            color: VGAColor::new(Color::White, Color::Black),
            buffer: VGABuffer {
                chars: [[VGABufferEntry {
                    ascii_char: b' ',
                    color: VGAColor::new(Color::White, Color::Black),
                }; WIDTH]; HEIGHT],
            },
            output: unsafe { &mut *(BUF_ADDR as *mut VGABuffer) },
        }
    }

    pub fn write_byte(&mut self, b: u8) {
        match b {
            b'\n' => self.newline(),
            b'\r' => self.col = 0,
            b'\t' => self.col += 4,
            b => {
                if self.col >= WIDTH {
                    self.newline();
                }

                self.buffer.chars[self.row][self.col] = VGABufferEntry {
                    ascii_char: b,
                    color: self.color,
                };

                self.col += 1;
            }
        }
    }

    pub fn write_str(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                // printable ASCII byte or newline
                0x20..=0x7e | b'\n' | b'\r' | b'\t' => self.write_byte(byte),
                // not part of printable ASCII range
                _ => self.write_byte(0xfe),
            }
        }
    }

    pub fn flush(&mut self) {
        for row in 0..HEIGHT {
            for col in 0..WIDTH {
                let entry = self.buffer.chars[row][col];
                let addr = addr_of_mut!(self.output.chars[row][col]);
                unsafe {
                    addr.write_volatile(entry);
                }
            }
        }

        // Video interrupt to cursor to current position
        unsafe {
            asm!("push rbx", "mov bx, 0x0", "pop rbx", in("ax") 0x02, in("dx") self.row << 8 | self.col);
        }
    }

    fn newline(&mut self) {
        for row in 0..HEIGHT - 1 {
            for col in 0..WIDTH {
                let entry = self.buffer.chars[row + 1][col];
                self.buffer.chars[row][col] = entry;
            }
        }

        self.clear_row(HEIGHT - 1);
        self.col = 0;
    }

    fn clear_row(&mut self, row: usize) {
        let blank = VGABufferEntry {
            ascii_char: b' ',
            color: self.color,
        };

        for col in 0..WIDTH {
            self.buffer.chars[row][col] = blank;
        }
    }
}

impl fmt::Write for VGAWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_str(s);
        Ok(())
    }
}

lazy_static! {
    pub static ref WRITER: Mutex<VGAWriter> = Mutex::new(VGAWriter::new());
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vgabuf::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    interrupts::without_interrupts(|| {
        let mut writer = WRITER.lock();
        writer.write_fmt(args).unwrap();
        writer.flush();
    });
}

pub fn flush() {
    interrupts::without_interrupts(|| WRITER.lock().flush());
}

#[test_case]
fn test_print() {
    println!("Printning to VGA buffer");
}

#[test_case]
fn test_print_loads() {
    for _ in 0..200 {
        println!("Printning to VGA buffer");
    }
}

#[cfg(test)]
fn get_char_at(row: usize, col: usize) -> char {
    WRITER.lock().output.chars[row][col].ascii_char as char
}

#[test_case]
fn test_println() {
    let s = "Check that this string is actually printed to the VGA buffer";
    println!("{}", s);
    for (i, c) in s.chars().enumerate() {
        assert_eq!(get_char_at(HEIGHT - 2, i), c);
    }
}

#[test_case]
fn test_wrapping() {
    use core::fmt::Write;

    let loops = 10;
    let s = "Repeating this string should wrap around the VGA buffer";
    interrupts::without_interrupts(|| {
        let mut writer = WRITER.lock();
        writeln!(writer).unwrap();
        for _ in 0..loops {
            write!(writer, "{}", s).unwrap();
        }
        writeln!(writer).unwrap();
        let start_row = HEIGHT - 2 - s.len() * loops / WIDTH;
        for (i, c) in s.chars().cycle().take(s.len() * loops).enumerate() {
            let row = start_row + i / WIDTH;
            let col = i % WIDTH;
            assert_eq!(writer.output.chars[row][col].ascii_char as char, c);
        }
    })
}
