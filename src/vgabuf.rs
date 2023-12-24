use core::{
    fmt,
    ptr::{addr_of_mut, write_volatile},
};

use lazy_static::lazy_static;
use spin::Mutex;

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
    buffer: [[VGABufferEntry; WIDTH]; HEIGHT],
}

pub struct VGAWriter {
    col: usize,
    color: VGAColor,
    buffer: &'static mut VGABuffer,
}

impl VGAWriter {
    pub fn new() -> VGAWriter {
        VGAWriter {
            col: 0,
            color: VGAColor::new(Color::White, Color::Black),
            buffer: unsafe { &mut *(BUF_ADDR as *mut VGABuffer) },
        }
    }

    pub fn write_byte(&mut self, b: u8) {
        match b {
            b'\n' => self.newline(),
            b => {
                if self.col >= WIDTH {
                    self.newline();
                }

                let row = HEIGHT - 1;
                let col = self.col;
                let addr = addr_of_mut!(self.buffer.buffer[row][col]);

                unsafe {
                    write_volatile(
                        addr,
                        VGABufferEntry {
                            ascii_char: b,
                            color: self.color,
                        },
                    );
                }

                self.col += 1;
            }
        }
    }

    pub fn write_str(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                // printable ASCII byte or newline
                0x20..=0x7e | b'\n' => self.write_byte(byte),
                // not part of printable ASCII range
                _ => self.write_byte(0xfe),
            }
        }
    }

    fn newline(&mut self) {
        for row in 0..HEIGHT - 1 {
            for col in 0..WIDTH {
                let entry = self.buffer.buffer[row + 1][col];
                self.buffer.buffer[row][col] = entry;
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
            self.buffer.buffer[row][col] = blank;
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
    WRITER.lock().write_fmt(args).unwrap();
}
