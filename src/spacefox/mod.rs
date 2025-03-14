use core::slice::from_raw_parts;

use crate::{
    pci::audio_ac97::{music_loop::MusicLoop, AudioAc97},
    phys_alloc::PhysAllocator,
};
use music_data::WAV_DATA_SAMPLES;
use pluggable_interrupt_os::{
    println,
    vga_buffer::{is_drawable, plot, Color, ColorCode, BUFFER_HEIGHT, BUFFER_WIDTH},
};

mod music_data;

type Line = [i8; 4];
type LineBank = [Line; 100];

type WorldLine = [f32; 4];
type WorldLineBank = [WorldLine; 100];

pub struct SpaceFox<'a> {
    music: MusicLoop<'a>,
    even_lines: LineBank,
    odd_lines: LineBank,
    world: WorldLineBank,
}

impl<'a> SpaceFox<'a> {
    pub fn new(phys_alloc: &mut PhysAllocator, ac97: AudioAc97) -> Self {
        let music = MusicLoop::new(phys_alloc, &WAV_DATA_SAMPLES, ac97);

        let mut even_lines = [Default::default(); 100];
        even_lines[0] = [5, 5, 10, 10];

        Self {
            music,
            even_lines,
            odd_lines: [Default::default(); 100],
            world: [Default::default(); 100],
        }
    }

    pub fn start_game(&mut self) {
        self.music.play();
    }

    pub fn update(&mut self) {
        self.music.wind();

        let y1 = self.even_lines[0][1];

        self.even_lines[0][1] = (y1 + 1) % BUFFER_HEIGHT as i8;
    }

    pub fn draw(&self) {
        draw_lines(self.even_lines, '#');
    }
}

fn draw_lines(lb: LineBank, linechar: char) {
    let myplot = |x, y| {
        if x < 0 || y < 0 {
            return;
        }
        plot(
            linechar,
            y as usize,
            x as usize,
            ColorCode::new(Color::LightCyan, Color::Black),
        );
    };
    for l in lb {
        if l != [0, 0, 0, 0] {
            plot_line(l, myplot);
        }
    }
}

// Bresenham's line algorithm, adapted from:
// https://en.wikipedia.org/wiki/Bresenham%27s_line_algorithm

pub fn plot_line([mut x1, mut y1, x2, y2]: Line, mut plot: impl FnMut(i8, i8)) {
    let dx = x2.abs_diff(x1);
    let dy = y1.abs_diff(y1);
    let sx = if x1 < x2 { 1 } else { -1 };
    let sy = if y1 < y2 { 1 } else { -1 };
    let mut error = dx + dy;

    loop {
        plot(x1, y1);
        if x1 == x2 && y1 == y2 {
            break;
        }
        let e2 = 2 * error;
        if e2 >= dy {
            if x1 == x2 {
                break;
            }
            error += dy;
            x1 += sx;
        }
        if e2 <= dx {
            if y1 == y2 {
                break;
            }
            error += dx;
            y1 += sy;
        }
    }
}
