use crate::{
    pci::audio_ac97::{music_loop::MusicLoop, AudioAc97},
    phys_alloc::PhysAllocator,
};
use music_data::WAV_DATA_SAMPLES;
use pc_keyboard::DecodedKey;
use pluggable_interrupt_os::{
    print, println,
    vga_buffer::{clear_screen, plot, Color, ColorCode, BUFFER_HEIGHT, BUFFER_WIDTH},
};

mod music_data;

type Line = [i8; 6];
type LineBank = [Line; 100];

const fn v(value: [f32; 3]) -> Vec3f {
    Vec3f {
        x: value[0],
        y: value[1],
        z: value[2],
    }
}

#[derive(Default, Clone, Copy)]
struct Vec3f {
    x: f32,
    y: f32,
    z: f32,
}

impl Vec3f {
    const fn mirx(&self) -> Self {
        Self {
            x: -self.x,
            y: self.y,
            z: self.z,
        }
    }
}

type World = [Model; 30];

#[derive(Default, Clone, Copy)]
struct Model {
    prims: &'static [Prim],
    pos: Vec3f,
    scale: f32,
}

const SHIPV1: Vec3f = v([-1.361523, -0.532862, -6.396634]);
const SHIPV2: Vec3f = v([0.000000, 0.299368, 0.285550]);
const SHIPV3: Vec3f = v([0.000000, -0.492211, 0.303217]);
const SHIPV4: Vec3f = v([0.000000, -0.036415, -1.398604]);
const SHIPF1: Prim = Prim::Tri(SHIPV1, SHIPV2, SHIPV4);
const SHIPF2: Prim = Prim::Tri(SHIPV3, SHIPV1, SHIPV4);
const SHIPF3: Prim = Prim::Tri(SHIPV1.mirx(), SHIPV2.mirx(), SHIPV4.mirx());
const SHIPF4: Prim = Prim::Tri(SHIPV3.mirx(), SHIPV1.mirx(), SHIPV4.mirx());
const SHIP_TRIS: &[Prim] = &[SHIPF1, SHIPF2, SHIPF3, SHIPF4];

#[derive(Default, Clone, Copy)]
enum Prim {
    #[default]
    Noop,
    Tri(Vec3f, Vec3f, Vec3f),
    Quad(Vec3f, Vec3f, Vec3f, Vec3f),
}

pub struct SpaceFox<'a> {
    music: MusicLoop<'a>,
    b: usize,
    lines: [LineBank; 2],
    world: World,
}

impl<'a> SpaceFox<'a> {
    pub fn new(phys_alloc: &mut PhysAllocator, ac97: AudioAc97) -> Self {
        let music = MusicLoop::new(phys_alloc, &WAV_DATA_SAMPLES, ac97);

        let mut world = [Default::default(); 30];
        world[0] = Model {
            prims: SHIP_TRIS,
            pos: v([0.0, -1.0, 15.0]),
            scale: 1.0,
        };

        Self {
            music,
            lines: [[Default::default(); 100], [Default::default(); 100]],
            b: 0,
            world,
        }
    }

    fn swap_buffer(&mut self) {
        self.b ^= 1
    }

    pub fn start_game(&mut self) {
        // self.music.play();
        let c = ColorCode::new(Color::Yellow, Color::LightGray);
        for x in 0..BUFFER_WIDTH {
            plot('@', x, 0, c);
            plot('@', x, BUFFER_HEIGHT - 1, c);
        }
        for y in 0..BUFFER_HEIGHT {
            plot('@', 0, y, c);
            plot('@', BUFFER_WIDTH - 1, y, c);
        }
    }

    pub fn update(&mut self) {
        self.music.wind();

        // self.world[0].pos.x += 0.1;

        let mut next_line = 0;
        for Model { prims, pos, scale } in self.world {
            for p in prims {
                match p {
                    Prim::Noop => {}
                    Prim::Tri(p1, p2, p3) => {
                        if next_line + 3 >= self.lines[0].len() {
                            break;
                        }
                        // if [p1.z, p2.z, p3.z].iter().any(|z| *z <= 0.0) {
                        //     continue;
                        // }

                        const ASPECT_X: f32 = BUFFER_WIDTH as f32;
                        const ASPECT_Y: f32 = -(BUFFER_HEIGHT as f32);
                        const XOFF: f32 = BUFFER_WIDTH as f32 / 2.0;
                        const YOFF: f32 = BUFFER_HEIGHT as f32 / 2.0;

                        let modl = |p: &Vec3f| -> Vec3f {
                            Vec3f {
                                x: p.x + pos.x,
                                y: p.y + pos.y,
                                z: p.z + pos.z,
                            }
                        };
                        let persp = |p: Vec3f| -> (f32, f32, f32) {
                            (
                                p.x * scale / p.z * ASPECT_X + XOFF,
                                p.y * scale / p.z * ASPECT_Y + YOFF,
                                p.z,
                            )
                        };

                        // print!("[{}]", p1.z + pos.z);

                        let i = |p: &Vec3f| -> (i8, i8, i8) {
                            let (x, y, z) = persp(modl(p));
                            (x as i8, y as i8, z as i8)
                        };

                        let (x1, y1, z1) = i(p1);
                        let (x2, y2, z2) = i(p2);
                        let (x3, y3, z3) = i(p3);

                        if x1 < 0 || x1 > BUFFER_WIDTH as i8 {
                            continue;
                        }
                        if x2 < 0 || x2 > BUFFER_WIDTH as i8 {
                            continue;
                        }
                        if x2 < 0 || x2 > BUFFER_WIDTH as i8 {
                            continue;
                        }

                        // println!("({},{})({},{})({},{})", x1, y1, x2, y2, x3, y3);

                        // fn crappy_ln(x: f32) -> f32 {
                        //     // 1.0 - 1.0 / x + x / 8.0
                        //     // 3.41333 - 3.41333 / x + 0.853333 * x
                        //     x
                        // }
                        //
                        // let z1 = crappy_ln(p1.z + pos.z) as i8;
                        // let z2 = crappy_ln(p2.z + pos.z) as i8;
                        // let z3 = crappy_ln(p3.z + pos.z) as i8;

                        self.lines[self.b][next_line + 0] = [x1, y1, z1, x2, y2, z2];
                        self.lines[self.b][next_line + 1] = [x2, y2, z2, x3, y3, z3];
                        self.lines[self.b][next_line + 2] = [x3, y3, z3, x1, y1, z1];

                        next_line += 3;
                    }
                    Prim::Quad(_, _, _, _) => todo!(),
                }
            }
        }
    }

    pub fn draw(&mut self) {
        draw_lines(&self.lines[self.b ^ 1], ' ');
        draw_lines(&self.lines[self.b], '#');
        self.swap_buffer();
        // clear_screen();

        // let myplot = |x, y| {
        //     if x < 0 || y < 0 {
        //         return;
        //     }
        //     plot(
        //         '|',
        //         y as usize,
        //         x as usize,
        //         ColorCode::new(Color::LightCyan, Color::Black),
        //     );
        // };
        // plot_line(&[5, 7, 20, 20], myplot);
    }

    pub fn key(&mut self, k: DecodedKey) {
        match k {
            // DecodedKey::RawKey(key_code) => todo!(),
            DecodedKey::Unicode('a') => self.world[0].pos.x -= 1.0,
            DecodedKey::Unicode('d') => self.world[0].pos.x += 1.0,
            DecodedKey::Unicode('w') => self.world[0].pos.y += 1.0,
            DecodedKey::Unicode('s') => self.world[0].pos.y -= 1.0,
            DecodedKey::Unicode('u') => self.world[0].scale += 0.1,
            DecodedKey::Unicode('p') => self.world[0].scale -= 0.1,
            DecodedKey::Unicode('q') => self.world[0].pos.z += 1.0,
            DecodedKey::Unicode('e') => self.world[0].pos.z -= 1.0,
            // DecodedKey::Unicode(_) => todo!(),
            _ => {}
        }
    }
}

fn draw_lines(lb: &LineBank, linechar: char) {
    let myplot = |x, y, c| {
        if x < 0 || y < 0 || x >= BUFFER_WIDTH as i32 || y >= BUFFER_HEIGHT as i32 {
            return;
        }
        let c = if linechar == ' ' { ' ' } else { c };
        plot(
            c,
            x as usize,
            y as usize,
            ColorCode::new(Color::LightCyan, Color::Black),
        );
    };
    for l in lb {
        if l != &[0, 0, 0, 0, 0, 0] {
            plot_line(l, myplot);
        }
    }
}

// Bresenham's line algorithm, adapted from:
// https://en.wikipedia.org/wiki/Bresenham%27s_line_algorithm

pub fn plot_line(&[x1, y1, z1, x2, y2, z2]: &Line, mut plot: impl FnMut(i32, i32, char)) {
    let mut x1 = x1 as i32;
    let mut y1 = y1 as i32;
    let x2 = x2 as i32;
    let y2 = y2 as i32;

    let dx = i32::abs(x2 - x1);
    let dy = -i32::abs(y2 - y1);
    let sx = if x1 < x2 { 1 } else { -1 };
    let sy = if y1 < y2 { 1 } else { -1 };
    let mut error = dx + dy;

    loop {
        let percentage = i32::abs(x2 - x1) as f32 / (dx as f32 + 1.0);
        let depth = (z1 as f32 * percentage) + (z2 as f32 * (1.0 - percentage));
        let i = (depth) / 30.0;
        let i = (i * GRAD.len() as f32)
            .max(0.0)
            .min(GRAD.len() as f32 - 1.0);

        // const GRAD: &[char] = &['.', ':', '*', '#', '$', '@'];
        const GRAD: &[char] = &['1', '2', '3', '4', '5', '6', '7', '8', '9'];
        plot(x1, y1, GRAD[i as usize]);
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
