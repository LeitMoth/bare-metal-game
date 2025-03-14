use crate::{
    pci::audio_ac97::{music_loop::MusicLoop, AudioAc97},
    phys_alloc::PhysAllocator,
};
use music_data::WAV_DATA_SAMPLES;
use pc_keyboard::DecodedKey;
use pluggable_interrupt_os::{
    println,
    vga_buffer::{clear_screen, plot, Color, ColorCode, BUFFER_HEIGHT, BUFFER_WIDTH},
};

mod music_data;

type Line = [i8; 7];
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
// const SHIPV2: Vec3f = v([0.000000, 0.299368, 0.285550]);
const SHIPV2: Vec3f = v([0.000000, 1.5, 0.285550]);
const SHIPV3: Vec3f = v([0.000000, -0.492211, 0.303217]);
const SHIPV4: Vec3f = v([0.000000, -0.036415, -1.398604]);
const SHIPF1: Prim = Prim::Tri(SHIPV1, SHIPV2, SHIPV4);
const SHIPF2: Prim = Prim::Tri(SHIPV3, SHIPV1, SHIPV4);
const SHIPF3: Prim = Prim::Tri(SHIPV1.mirx(), SHIPV2.mirx(), SHIPV4.mirx());
const SHIPF4: Prim = Prim::Tri(SHIPV3.mirx(), SHIPV1.mirx(), SHIPV4.mirx());
const SHIP_TRIS: &[Prim] = &[SHIPF1, SHIPF2, SHIPF3, SHIPF4];

const BLOCKHEIGHT: f32 = 5.0;
const BLOCKDEPTH: f32 = 8.0;
const BLOCKV25: Vec3f = v([1.000000, -1.000000, BLOCKDEPTH]);
const BLOCKV26: Vec3f = v([1.000000, BLOCKHEIGHT, BLOCKDEPTH]);
const BLOCKV27: Vec3f = v([1.000000, -1.000000, 1.000000]);
const BLOCKV28: Vec3f = v([1.000000, BLOCKHEIGHT, 1.000000]);
const BLOCKV29: Vec3f = v([-1.000000, -1.000000, BLOCKDEPTH]);
const BLOCKV30: Vec3f = v([-1.000000, BLOCKHEIGHT, BLOCKDEPTH]);
const BLOCKV31: Vec3f = v([-1.000000, -1.000000, 1.000000]);
const BLOCKV32: Vec3f = v([-1.000000, BLOCKHEIGHT, 1.000000]);
const BLOCKF1: Prim = Prim::Quad(BLOCKV25, BLOCKV26, BLOCKV28, BLOCKV27);
const BLOCKF2: Prim = Prim::Quad(BLOCKV31, BLOCKV32, BLOCKV30, BLOCKV29);
const BLOCKF3: Prim = Prim::Quad(BLOCKV29, BLOCKV30, BLOCKV26, BLOCKV25);
const BLOCKF4: Prim = Prim::Quad(BLOCKV32, BLOCKV28, BLOCKV26, BLOCKV30);
const BLOCK_QUADS: &[Prim] = &[BLOCKF1, BLOCKF2, BLOCKF3, BLOCKF4];

#[derive(Default, Clone, Copy)]
enum Prim {
    #[default]
    Noop,
    Tri(Vec3f, Vec3f, Vec3f),
    Quad(Vec3f, Vec3f, Vec3f, Vec3f),
}

pub struct Game<'a> {
    music: MusicLoop<'a>,
    music_started: bool,
    state: GameState,
    random: u64,
}

enum GameState {
    Menu { first_draw: bool, need_start: bool },
    SpaceFox(SpaceFox),
    GameOver { first_draw: bool, timer: u16 },
}

pub struct SpaceFox {
    b: usize,
    lines: [LineBank; 2],
    end: [usize; 2],
    world: World,
    xvel: f32,
    yvel: f32,
}

const PLAYER: usize = 1;
const BLOCK: usize = 0;

impl<'a> Game<'a> {
    pub fn new(phys_alloc: &mut PhysAllocator, ac97: AudioAc97) -> Self {
        let music = MusicLoop::new(phys_alloc, &WAV_DATA_SAMPLES, ac97);
        Self {
            music,
            music_started: false,
            state: GameState::Menu {
                first_draw: true,
                need_start: false,
            },
            random: 0xDEADBEEF,
        }
    }

    fn rand(&mut self) -> u8 {
        self.random = 599u64.wrapping_mul(self.random).wrapping_add(4153) % 7919;

        return (self.random % 256) as u8;
    }

    pub fn tick(&mut self) {
        if self.music_started {
            self.music.wind();
        }
        let r = self.rand();
        match self.state {
            GameState::Menu {
                ref mut first_draw,
                ref mut need_start,
            } => {
                if *first_draw {
                    clear_screen();
                    // "    _, ._,     ._, . , "
                    // "\\./(_) (_      (_  |_| "
                    // "/'\\(_) (_) ____(_)   | "
                    println!(
                        "        {}        {}",
                        " __.            .___      ", "    _, ._,     ._, . , "
                    );
                    println!(
                        "        {}        {}",
                        "(__ ._  _. _. _ [__  _ \\./", "\\./(_) (_      (_  |_| "
                    );
                    println!(
                        "        {}        {}",
                        ".__)[_)(_](_.(/,|   (_)/'\\", "/'\\(_) (_) ____(_)   | "
                    );
                    println!("        {}        {}", "    |                     ", "");
                    println!();
                    println!();
                    println!();
                    println!("    SpaceFox x86_64");
                    println!();
                    println!("        Use WASD to move, and Space to brake");
                    println!("        Watch out for the red obstacles");
                    println!();
                    println!("        Press any key to play!");
                    println!();
                    println!();
                    println!();
                    println!();
                    *first_draw = false;
                }
                if *need_start {
                    clear_screen();
                    if !self.music_started {
                        self.music.play();
                        self.music_started = true;
                    }
                    self.state = GameState::SpaceFox(SpaceFox::new());
                }
            }
            GameState::SpaceFox(ref mut space_fox) => {
                if space_fox.update(r) {
                    space_fox.draw();
                } else {
                    self.state = GameState::GameOver {
                        first_draw: true,
                        timer: 50,
                    };
                }
            }
            GameState::GameOver {
                ref mut first_draw,
                ref mut timer,
            } => {
                if *first_draw {
                    clear_screen();
                    println!("           _______                      _______                  ");
                    println!("          |   _   .---.-.--------.-----|   _   .--.--.-----.----.");
                    println!("          |.  |___|  _  |        |  -__|.  |   |  |  |  -__|   _|");
                    println!("          |.  |   |___._|__|__|__|_____|.  |   |\\___/|_____|__|  ");
                    println!("          |:  1   |                    |:  1   |                 ");
                    println!("          |::.. . |                    |::.. . |                 ");
                    println!("          `-------'                    `-------'                 ");
                    println!();
                    println!();
                    println!();
                    println!("        Game Over! You hit a tower!");
                    println!();
                    println!();
                    println!();
                    println!();
                    println!();
                    println!();
                    *first_draw = false;
                }
                *timer -= 1;
                if *timer == 0 {
                    self.state = GameState::Menu {
                        first_draw: true,
                        need_start: false,
                    };
                }
            }
        }
    }

    pub fn key(&mut self, k: DecodedKey) {
        // hopefully we can get better randomness this way
        match k {
            DecodedKey::RawKey(key_code) => {
                self.random = self
                    .random
                    .wrapping_mul((key_code as u64).wrapping_sub(7))
                    .wrapping_add(key_code as u64);
            }
            DecodedKey::Unicode(c) => {
                self.random = self
                    .random
                    .wrapping_mul((c as u64).wrapping_sub(72))
                    .wrapping_add(c as u64);
            }
        }
        match self.state {
            GameState::Menu {
                ref mut need_start, ..
            } => *need_start = true,
            GameState::SpaceFox(ref mut space_fox) => space_fox.key(k),
            GameState::GameOver { .. } => {}
        }
    }
}

const GRAD_HOR: &[u8] = "#==----==#".as_bytes();

impl SpaceFox {
    pub fn new() -> Self {
        for x in 0..BUFFER_WIDTH {
            let c = GRAD_HOR[x / 8];
            plot(
                c as char,
                x,
                12,
                ColorCode::new(Color::LightGray, Color::Black),
            );
        }

        let mut world = [Default::default(); 30];

        world[PLAYER] = Model {
            prims: SHIP_TRIS,
            pos: v([0.0, -3.0, 15.0]),
            scale: 1.0,
        };

        world[BLOCK] = Model {
            prims: BLOCK_QUADS,
            pos: v([-3.0, -3.0, 200.0]),
            scale: 2.0,
        };

        Self {
            lines: [[Default::default(); 100], [Default::default(); 100]],
            end: [0, 0],
            b: 0,
            world,
            xvel: 0.0,
            yvel: 0.0,
        }
    }

    fn swap_buffer(&mut self) {
        self.b ^= 1
    }

    pub fn update(&mut self, rand: u8) -> bool {
        {
            let z = self.world[BLOCK].pos.z;
            self.world[BLOCK].pos.z -= (0.01 * z + 3.0).max(1.0);
            if self.world[BLOCK].pos.z < -5.0 {
                self.world[BLOCK].pos.z = 200.0;
                let interp = ((rand as f32) / 255.0) * 6.0 - 3.0;
                self.world[BLOCK].pos.x = interp;
            }
        }

        {
            let x = &mut self.world[PLAYER].pos.x;
            *x += self.xvel;
            if *x < -3.0 {
                *x = -3.0;
            }
            if *x > 3.0 {
                *x = 3.0;
            }

            let y = &mut self.world[PLAYER].pos.y;
            *y += self.yvel;
            if *y < -4.0 {
                *y = -4.0;
            }
            if *y > 5.0 {
                *y = 5.0;
            }
        }

        let mut next_line = 0;
        for Model { prims, pos, scale } in self.world {
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
                if p.z.abs() < 0.01 {
                    (0.0, 0.0, 0.0)
                } else {
                    (
                        p.x * scale / p.z * ASPECT_X + XOFF,
                        p.y * scale / p.z * ASPECT_Y + YOFF,
                        p.z,
                    )
                }
            };

            let i = |p: &Vec3f| -> (i8, i8, i8) {
                let (x, y, z) = persp(modl(p));
                (x as i8, y as i8, z as i8)
            };

            for p in prims {
                match p {
                    Prim::Noop => {}
                    Prim::Tri(p1, p2, p3) => {
                        if next_line + 3 >= self.lines[0].len() {
                            break;
                        }

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

                        self.lines[self.b][next_line + 0] = [x1, y1, z1, x2, y2, z2, 0];
                        self.lines[self.b][next_line + 1] = [x2, y2, z2, x3, y3, z3, 0];
                        self.lines[self.b][next_line + 2] = [x3, y3, z3, x1, y1, z1, 0];

                        next_line += 3;
                    }
                    Prim::Quad(p1, p2, p3, p4) => {
                        if next_line + 4 >= self.lines[0].len() {
                            break;
                        }

                        let (x1, y1, z1) = i(p1);
                        let (x2, y2, z2) = i(p2);
                        let (x3, y3, z3) = i(p3);
                        let (x4, y4, z4) = i(p4);

                        if x1 < 0 || x1 > BUFFER_WIDTH as i8 {
                            continue;
                        }
                        if x2 < 0 || x2 > BUFFER_WIDTH as i8 {
                            continue;
                        }
                        if x2 < 0 || x2 > BUFFER_WIDTH as i8 {
                            continue;
                        }
                        if x4 < 0 || x4 > BUFFER_WIDTH as i8 {
                            continue;
                        }

                        self.lines[self.b][next_line + 0] = [x1, y1, z1, x2, y2, z2, 1];
                        self.lines[self.b][next_line + 1] = [x2, y2, z2, x3, y3, z3, 1];
                        self.lines[self.b][next_line + 2] = [x3, y3, z3, x4, y4, z4, 1];
                        self.lines[self.b][next_line + 3] = [x4, y4, z4, x1, y1, z1, 1];

                        next_line += 4;
                    }
                }
            }
        }

        self.end[self.b] = next_line;

        let p1 = self.world[BLOCK].pos;
        let p2 = self.world[PLAYER].pos;
        let dx = p1.x - p2.x;
        // let dy = p1.y - p2.y;
        let dz = p1.z - p2.z;

        dx * dx + dz * dz > 1.0
    }

    pub fn draw(&mut self) {
        let d = self.b ^ 1;
        clear_lines(&self.lines[d], self.end[d]);
        plot_line(10, 24, 40, 10, '/');
        plot_line(70, 24, 40, 10, '\\');
        self.end[d] = 0;
        draw_lines(&self.lines[self.b], self.end[self.b]);
        self.swap_buffer();
    }

    pub fn key(&mut self, k: DecodedKey) {
        const XSPEED: f32 = 0.7;
        match k {
            DecodedKey::Unicode('a') => self.xvel = -XSPEED,
            DecodedKey::Unicode('d') => self.xvel = XSPEED,
            DecodedKey::Unicode('w') => self.yvel = XSPEED,
            DecodedKey::Unicode('s') => self.yvel = -XSPEED,
            DecodedKey::Unicode(' ') => {
                self.xvel = 0.0;
                self.yvel = 0.0;
            }
            _ => {}
        }
    }
}

fn clear_lines(lb: &LineBank, end: usize) {
    for l in &lb[0..end] {
        if l != &[0, 0, 0, 0, 0, 0, 0] {
            clear_line(l);
        }
    }
}

fn draw_lines(lb: &LineBank, end: usize) {
    for l in &lb[0..end] {
        if l != &[0, 0, 0, 0, 0, 0, 0] {
            plot_line_depth(l);
        }
    }
}

// Bresenham's line algorithm, adapted from:
// https://en.wikipedia.org/wiki/Bresenham%27s_line_algorithm

pub fn plot_line_depth(&[x1, y1, z1, x2, y2, z2, cbit]: &Line) {
    let color = if cbit == 0 {
        Color::LightCyan
    } else {
        Color::Red
    };
    let mut x1 = x1 as i32;
    let mut y1 = y1 as i32;
    let x2 = x2 as i32;
    let y2 = y2 as i32;

    let dx = i32::abs(x2 - x1);
    let dy = -i32::abs(y2 - y1);
    let sx = if x1 < x2 { 1 } else { -1 };
    let sy = if y1 < y2 { 1 } else { -1 };
    let mut error = dx + dy;

    for _ in 0..24 {
        let percentage = i32::abs(x2 - x1) as f32 / (dx as f32 + 1.0);
        let depth = (z1 as f32 * percentage) + (z2 as f32 * (1.0 - percentage));
        let i = (depth) / 60.0;
        let i = (i * GRAD.len() as f32)
            .max(0.0)
            .min(GRAD.len() as f32 - 1.0);

        // const GRAD: &[char] = &['1', '2', '3', '4', '5', '6', '7', '8', '9'];
        const GRAD: &[char] = &['#', '#', '#', '@', '*', ',', ',', '.'];
        let c = if cbit == 1 { GRAD[i as usize] } else { '*' };

        if x1 >= 0 && x1 < BUFFER_WIDTH as i32 && y1 >= 0 && y1 < BUFFER_HEIGHT as i32 {
            plot(
                c,
                x1 as usize,
                y1 as usize,
                ColorCode::new(color, Color::Black),
            );
        }
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

pub fn plot_line(mut x1: i32, mut y1: i32, x2: i32, y2: i32, c: char) {
    let dx = i32::abs(x2 - x1);
    let dy = -i32::abs(y2 - y1);
    let sx = if x1 < x2 { 1 } else { -1 };
    let sy = if y1 < y2 { 1 } else { -1 };
    let mut error = dx + dy;

    for _ in 0..24 {
        if x1 >= 0 && x1 < BUFFER_WIDTH as i32 && y1 >= 0 && y1 < BUFFER_HEIGHT as i32 {
            plot(
                c,
                x1 as usize,
                y1 as usize,
                ColorCode::new(Color::DarkGray, Color::Black),
            );
        }
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

pub fn clear_line(&[x1, y1, _, x2, y2, _, _]: &Line) {
    let mut x1 = x1 as i32;
    let mut y1 = y1 as i32;
    let x2 = x2 as i32;
    let y2 = y2 as i32;

    let dx = i32::abs(x2 - x1);
    let dy = -i32::abs(y2 - y1);
    let sx = if x1 < x2 { 1 } else { -1 };
    let sy = if y1 < y2 { 1 } else { -1 };
    let mut error = dx + dy;

    for _ in 0..24 {
        if x1 >= 0 && x1 < BUFFER_WIDTH as i32 && y1 >= 0 && y1 < BUFFER_HEIGHT as i32 {
            if y1 == 12 {
                let c = GRAD_HOR[x1 as usize / 8];
                plot(
                    c as char,
                    x1 as usize,
                    y1 as usize,
                    ColorCode::new(Color::LightGray, Color::Black),
                );
            } else {
                plot(
                    ' ',
                    x1 as usize,
                    y1 as usize,
                    ColorCode::new(Color::Black, Color::Black),
                );
            }
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
