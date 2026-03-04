use bootloader_api::info::{FrameBuffer, FrameBufferInfo, PixelFormat};
use core::fmt;
use font8x8::UnicodeFonts;
use spin::Mutex;

const CHAR_WIDTH: usize = 8;
const CHAR_HEIGHT: usize = 8;

static WRITER: Mutex<Option<FrameBufferWriter>> = Mutex::new(None);

pub fn init(framebuffer: &'static mut FrameBuffer) {
    let info = framebuffer.info();
    let buffer = framebuffer.buffer_mut();
    let mut writer = FrameBufferWriter::new(buffer, info);
    writer.clear();
    *WRITER.lock() = Some(writer);
}

pub fn clear() {
    if let Some(writer) = WRITER.lock().as_mut() {
        writer.clear();
    }
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments<'_>) {
    use core::fmt::Write;

    if let Some(writer) = WRITER.lock().as_mut() {
        let _ = writer.write_fmt(args);
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        $crate::framebuffer::_print(core::format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! println {
    () => {
        $crate::print!("\n")
    };
    ($($arg:tt)*) => {{
        $crate::framebuffer::_print(core::format_args!("{}\n", core::format_args!($($arg)*)));
    }};
}

struct FrameBufferWriter {
    buffer: &'static mut [u8],
    info: FrameBufferInfo,
    column: usize,
    row: usize,
}

impl FrameBufferWriter {
    fn new(buffer: &'static mut [u8], info: FrameBufferInfo) -> Self {
        Self {
            buffer,
            info,
            column: 0,
            row: 0,
        }
    }

    fn clear(&mut self) {
        self.buffer.fill(0);
        self.column = 0;
        self.row = 0;
    }

    fn max_columns(&self) -> usize {
        self.info.width / CHAR_WIDTH
    }

    fn max_rows(&self) -> usize {
        self.info.height / CHAR_HEIGHT
    }

    fn write_renderable(&mut self, character: char) {
        if self.max_columns() == 0 || self.max_rows() == 0 {
            return;
        }

        if self.column >= self.max_columns() {
            self.new_line();
        }

        if self.row >= self.max_rows() {
            self.scroll_one_row();
            self.row = self.max_rows() - 1;
        }

        let glyph = font8x8::BASIC_FONTS.get(character).unwrap_or([0; 8]);
        let pixel_x = self.column * CHAR_WIDTH;
        let pixel_y = self.row * CHAR_HEIGHT;
        self.draw_glyph(pixel_x, pixel_y, glyph);
        self.column += 1;
    }

    fn new_line(&mut self) {
        self.column = 0;
        self.row += 1;

        if self.row >= self.max_rows() {
            self.scroll_one_row();
            self.row = self.max_rows().saturating_sub(1);
        }
    }

    fn scroll_one_row(&mut self) {
        let row_bytes = self.info.stride * self.info.bytes_per_pixel;
        let char_row_bytes = row_bytes * CHAR_HEIGHT;
        if char_row_bytes == 0 || char_row_bytes >= self.buffer.len() {
            self.buffer.fill(0);
            return;
        }

        self.buffer.copy_within(char_row_bytes.., 0);
        let start = self.buffer.len() - char_row_bytes;
        self.buffer[start..].fill(0);
    }

    fn draw_glyph(&mut self, origin_x: usize, origin_y: usize, glyph: [u8; 8]) {
        for (glyph_y, row_bits) in glyph.iter().copied().enumerate() {
            for glyph_x in 0..8 {
                let on = row_bits & (1 << glyph_x) != 0;
                self.set_pixel(origin_x + glyph_x, origin_y + glyph_y, on);
            }
        }
    }

    fn set_pixel(&mut self, x: usize, y: usize, on: bool) {
        if x >= self.info.width || y >= self.info.height {
            return;
        }

        let bytes_per_pixel = self.info.bytes_per_pixel;
        let offset = (y * self.info.stride + x) * bytes_per_pixel;
        if offset + bytes_per_pixel > self.buffer.len() {
            return;
        }

        let color = if on { 0xff } else { 0x00 };
        let pixel = &mut self.buffer[offset..offset + bytes_per_pixel];
        match self.info.pixel_format {
            PixelFormat::Rgb => {
                pixel[0] = color;
                if bytes_per_pixel > 1 {
                    pixel[1] = color;
                }
                if bytes_per_pixel > 2 {
                    pixel[2] = color;
                }
                if bytes_per_pixel > 3 {
                    pixel[3] = 0x00;
                }
            }
            PixelFormat::Bgr => {
                pixel[0] = color;
                if bytes_per_pixel > 1 {
                    pixel[1] = color;
                }
                if bytes_per_pixel > 2 {
                    pixel[2] = color;
                }
                if bytes_per_pixel > 3 {
                    pixel[3] = 0x00;
                }
            }
            PixelFormat::U8 => {
                pixel[0] = color;
            }
            _ => {
                pixel.fill(0x00);
            }
        }
    }

    fn backspace(&mut self) {
        if self.max_columns() == 0 || self.max_rows() == 0 {
            return;
        }

        if self.column > 0 {
            self.column -= 1;
        } else if self.row > 0 {
            self.row -= 1;
            self.column = self.max_columns().saturating_sub(1);
        } else {
            return;
        }

        let pixel_x = self.column * CHAR_WIDTH;
        let pixel_y = self.row * CHAR_HEIGHT;
        self.draw_glyph(pixel_x, pixel_y, [0; 8]);
    }
}

impl fmt::Write for FrameBufferWriter {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for character in text.chars() {
            match character {
                '\n' => self.new_line(),
                '\r' => self.column = 0,
                '\u{8}' => self.backspace(),
                '\t' => {
                    for _ in 0..4 {
                        self.write_renderable(' ');
                    }
                }
                _ => {
                    let printable = if character.is_ascii() { character } else { '?' };
                    self.write_renderable(printable);
                }
            }
        }
        Ok(())
    }
}
