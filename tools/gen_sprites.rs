//! Sprite Sheet Generator for ScreenBuddy
//!
//! Generates idle, walk, fly, sleep, and celebrate animation frames
//! for all creature types.

use image::{ImageBuffer, Rgba};
use std::f32::consts::PI;

const SZ: u32 = 64;
const SC: u32 = 4;

fn px(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, x: i32, y: i32, c: [u8; 4]) {
    if x < 0 || y < 0 {
        return;
    }
    let px = x as u32 * SC;
    let py = y as u32 * SC;
    for dy in 0..SC {
        for dx in 0..SC {
            if px + dx < img.width() && py + dy < img.height() {
                img.put_pixel(px + dx, py + dy, Rgba(c));
            }
        }
    }
}

// ============================================================================
// COMPANION BIRD
// ============================================================================

fn draw_bird_idle(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let bob = (phase * PI * 2.0).sin() * 1.5;
    draw_bird_body(img, ox, bob, 0, 0);
}

fn draw_bird_walk(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let bob = (phase * PI * 4.0).sin() * 3.0;
    let leg_offset = (phase * PI * 4.0).cos() as i32 * 2;
    draw_bird_body(img, ox, bob, leg_offset, 0);
}

fn draw_bird_fly(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let bob = (phase * PI * 6.0).sin() * 4.0;
    let wing_flap = (phase * PI * 6.0).cos() as i32 * 4;
    draw_bird_body(img, ox, bob, 0, wing_flap);
}

fn draw_bird_sleep(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let breath = (phase * PI * 1.0).sin() * 0.5;
    draw_bird_body(img, ox, breath, 0, 0);
    px(img, 38 + ox, 8 + breath as i32, [0xFF, 0xFF, 0xFF, 255]);
    px(img, 42 + ox, 6 + breath as i32, [0xFF, 0xFF, 0xFF, 200]);
    px(img, 46 + ox, 4 + breath as i32, [0xFF, 0xFF, 0xFF, 150]);
}

fn draw_bird_celebrate(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let jump = (phase * PI * 8.0).abs().sin() * 6.0;
    let wing_flap = (phase * PI * 8.0).cos() as i32 * 5;
    draw_bird_body(img, ox, -jump, 0, wing_flap);
}

fn draw_bird_body(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    ox: i32,
    bob: f32,
    leg_offset: i32,
    wing_flap: i32,
) {
    let bob_y = bob as i32;

    for y in 20..44_i32 {
        for x in 16..44_i32 {
            let dx = x as f32 - 30.0;
            let dy = y as f32 - 32.0 + bob;
            let d = (dx * dx) / 144.0 + (dy * dy) / 64.0;
            if d < 1.0 {
                let c = if d > 0.85 {
                    [0x2D, 0x1B, 0x00, 255]
                } else {
                    [0xFF, 0x6B, 0x35, 255]
                };
                px(img, x + ox, y + bob_y, c);
            }
        }
    }

    for y in 24..36_i32 {
        for x in 8..20_i32 {
            let dx = x as f32 - 14.0;
            let dy = y as f32 - 30.0 + bob + wing_flap as f32;
            let d = (dx * dx) / 16.0 + (dy * dy) / 36.0;
            if d < 1.0 {
                px(img, x + ox, y + bob_y, [0xE0, 0x5A, 0x2E, 255]);
            }
        }
    }

    for y in 28..36_i32 {
        for x in 4..12_i32 {
            px(img, x + ox, y + bob_y, [0xCC, 0x4A, 0x20, 255]);
        }
    }

    for y in 12..24_i32 {
        for x in 24..38_i32 {
            let dx = x as f32 - 31.0;
            let dy = y as f32 - 18.0 + bob;
            let d = (dx * dx) / 36.0 + (dy * dy) / 36.0;
            if d < 1.0 {
                px(img, x + ox, y + bob_y, [0xFF, 0x6B, 0x35, 255]);
            }
        }
    }

    px(img, 30 + ox, 17 + bob_y, [0xFF, 0xFF, 0xFF, 255]);
    px(img, 31 + ox, 17 + bob_y, [0xFF, 0xFF, 0xFF, 255]);
    px(img, 30 + ox, 18 + bob_y, [0xFF, 0xFF, 0xFF, 255]);
    px(img, 31 + ox, 18 + bob_y, [0x00, 0x00, 0x00, 255]);
    px(img, 36 + ox, 19 + bob_y, [0xFF, 0xC0, 0x4D, 255]);
    px(img, 37 + ox, 19 + bob_y, [0xFF, 0xC0, 0x4D, 255]);
    px(img, 36 + ox, 20 + bob_y, [0xFF, 0xC0, 0x4D, 255]);
    px(
        img,
        24 + ox,
        43 + bob_y + leg_offset,
        [0xFF, 0xC0, 0x4D, 255],
    );
    px(
        img,
        25 + ox,
        43 + bob_y + leg_offset,
        [0xFF, 0xC0, 0x4D, 255],
    );
    px(
        img,
        34 + ox,
        43 + bob_y - leg_offset,
        [0xFF, 0xC0, 0x4D, 255],
    );
    px(
        img,
        35 + ox,
        43 + bob_y - leg_offset,
        [0xFF, 0xC0, 0x4D, 255],
    );
}

// ============================================================================
// DRAGON
// ============================================================================

fn draw_dragon_idle(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let bob = (phase * PI * 2.0).sin() * 1.5;
    draw_dragon_body(img, ox, bob, 0);
}

fn draw_dragon_walk(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let bob = (phase * PI * 4.0).sin() * 2.5;
    let leg_offset = (phase * PI * 4.0).cos() as i32 * 3;
    draw_dragon_body(img, ox, bob, leg_offset);
}

fn draw_dragon_fly(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let bob = (phase * PI * 6.0).sin() * 4.0;
    let wing = (phase * PI * 8.0).cos() as i32 * 8;
    draw_dragon_body(img, ox, bob, 0);
    // Wings
    for y in 16..32_i32 {
        for x in 0..16_i32 {
            px(img, x + ox, y + bob as i32 + wing, [0xFF, 0x44, 0x00, 200]);
        }
    }
    for y in 16..32_i32 {
        for x in 48..64_i32 {
            px(img, x + ox, y + bob as i32 - wing, [0xFF, 0x44, 0x00, 200]);
        }
    }
}

fn draw_dragon_sleep(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let breath = (phase * PI * 1.0).sin() * 0.5;
    draw_dragon_body(img, ox, breath, 0);
    // Zzz
    px(img, 50 + ox, 4, [0xFF, 0xFF, 0xFF, 255]);
    px(img, 54 + ox, 2, [0xFF, 0xFF, 0xFF, 200]);
    px(img, 58 + ox, 0, [0xFF, 0xFF, 0xFF, 150]);
}

fn draw_dragon_celebrate(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let jump = (phase * PI * 8.0).abs().sin() * 6.0;
    draw_dragon_body(img, ox, -jump, 0);
    // Fire breath
    for i in 0..5_i32 {
        px(
            img,
            58 + ox + i * 2,
            20 - i * 3,
            [0xFF, 0x88, 0x00, (255 - i * 40) as u8],
        );
    }
}

fn draw_dragon_body(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, bob: f32, leg_offset: i32) {
    let bob_y = bob as i32;

    // Body
    for y in 20..44_i32 {
        for x in 12..52_i32 {
            let dx = x as f32 - 32.0;
            let dy = y as f32 - 32.0 + bob;
            let d = (dx * dx) / 256.0 + (dy * dy) / 100.0;
            if d < 1.0 {
                let c = if d > 0.85 {
                    [0x22, 0x88, 0x22, 255]
                } else {
                    [0x33, 0xCC, 0x33, 255]
                };
                px(img, x + ox, y + bob_y, c);
            }
        }
    }

    // Head
    for y in 8..20_i32 {
        for x in 36..52_i32 {
            let dx = x as f32 - 44.0;
            let dy = y as f32 - 14.0 + bob;
            let d = (dx * dx) / 36.0 + (dy * dy) / 36.0;
            if d < 1.0 {
                px(img, x + ox, y + bob_y, [0x33, 0xCC, 0x33, 255]);
            }
        }
    }

    // Eyes (red)
    px(img, 42 + ox, 13 + bob_y, [0xFF, 0x00, 0x00, 255]);
    px(img, 48 + ox, 13 + bob_y, [0xFF, 0x00, 0x00, 255]);

    // Snout
    px(img, 50 + ox, 16 + bob_y, [0x22, 0x88, 0x22, 255]);
    px(img, 52 + ox, 16 + bob_y, [0x22, 0x88, 0x22, 255]);

    // Tail
    for y in 32..40_i32 {
        for x in 4..12_i32 {
            px(img, x + ox, y + bob_y, [0x22, 0x88, 0x22, 255]);
        }
    }

    // Legs
    for y in 44..52_i32 {
        for x in 16..24_i32 {
            px(img, x + ox, y + bob_y + leg_offset, [0x22, 0x88, 0x22, 255]);
        }
        for x in 40..48_i32 {
            px(img, x + ox, y + bob_y - leg_offset, [0x22, 0x88, 0x22, 255]);
        }
    }
}

// ============================================================================
// ROBO-CAT
// ============================================================================

fn draw_robo_cat_idle(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let bob = (phase * PI * 2.0).sin() * 1.0;
    draw_robo_cat_body(img, ox, bob, 0);
}

fn draw_robo_cat_walk(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let bob = (phase * PI * 4.0).sin() * 2.0;
    let leg_offset = (phase * PI * 4.0).cos() as i32 * 3;
    draw_robo_cat_body(img, ox, bob, leg_offset);
}

fn draw_robo_cat_fly(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let bob = (phase * PI * 6.0).sin() * 3.0;
    let jetpack = (phase * PI * 8.0).abs().sin() * 4.0;
    draw_robo_cat_body(img, ox, bob + jetpack, 0);
    for y in 44..56_i32 {
        for x in 20..28_i32 {
            if y as f32 > 44.0 + jetpack {
                px(img, x + ox, y, [0xFF, 0x44, 0x00, 200]);
            }
        }
    }
}

fn draw_robo_cat_sleep(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let breath = (phase * PI * 1.0).sin() * 0.5;
    draw_robo_cat_body(img, ox, breath, 0);
    px(img, 30 + ox, 4, [0x00, 0xFF, 0x00, 150]);
}

fn draw_robo_cat_celebrate(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let jump = (phase * PI * 8.0).abs().sin() * 5.0;
    draw_robo_cat_body(img, ox, -jump, 0);
    for i in 0..4 {
        px(
            img,
            ox + 10 + i * 12,
            10 + (i % 2) * 10,
            [0xFF, 0xFF, 0x00, 255],
        );
    }
}

fn draw_robo_cat_body(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    ox: i32,
    bob: f32,
    leg_offset: i32,
) {
    let bob_y = bob as i32;

    for y in 20..44_i32 {
        for x in 16..44_i32 {
            let dx = x as f32 - 30.0;
            let dy = y as f32 - 32.0 + bob;
            let d = (dx * dx) / 144.0 + (dy * dy) / 64.0;
            if d < 1.0 {
                let c = if d > 0.85 {
                    [0x33, 0x33, 0x33, 255]
                } else {
                    [0x66, 0x66, 0x66, 255]
                };
                px(img, x + ox, y + bob_y, c);
            }
        }
    }

    for y in 10..24_i32 {
        for x in 22..38_i32 {
            let dx = x as f32 - 30.0;
            let dy = y as f32 - 17.0 + bob;
            let d = (dx * dx) / 36.0 + (dy * dy) / 36.0;
            if d < 1.0 {
                px(img, x + ox, y + bob_y, [0x66, 0x66, 0x66, 255]);
            }
        }
    }

    px(img, 27 + ox, 16 + bob_y, [0x00, 0xFF, 0x88, 255]);
    px(img, 33 + ox, 16 + bob_y, [0x00, 0xFF, 0x88, 255]);
    px(img, 28 + ox, 16 + bob_y, [0x00, 0xFF, 0x88, 255]);
    px(img, 32 + ox, 16 + bob_y, [0x00, 0xFF, 0x88, 255]);
    px(img, 30 + ox, 6 + bob_y, [0x00, 0xFF, 0x88, 255]);
    px(img, 30 + ox, 7 + bob_y, [0x00, 0xFF, 0x88, 255]);

    for y in 30..36_i32 {
        for x in 4..12_i32 {
            px(img, x + ox, y + bob_y, [0x33, 0x33, 0x33, 255]);
        }
    }

    for y in 44..52_i32 {
        for x in 18..26_i32 {
            px(img, x + ox, y + bob_y + leg_offset, [0x33, 0x33, 0x33, 255]);
        }
        for x in 34..42_i32 {
            px(img, x + ox, y + bob_y - leg_offset, [0x33, 0x33, 0x33, 255]);
        }
    }
}

// ============================================================================
// SLIME KING
// ============================================================================

fn draw_slime_king_idle(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let wobble = (phase * PI * 2.0).sin() * 1.5;
    draw_slime_king_body(img, ox, wobble);
}

fn draw_slime_king_walk(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let wobble = (phase * PI * 4.0).sin() * 3.0;
    draw_slime_king_body(img, ox, wobble);
}

fn draw_slime_king_fly(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let wobble = (phase * PI * 6.0).sin() * 4.0;
    draw_slime_king_body(img, ox, wobble);
}

fn draw_slime_king_sleep(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let wobble = (phase * PI * 1.0).sin() * 0.5;
    draw_slime_king_body(img, ox, wobble);
    px(img, 38 + ox, 6, [0xFF, 0xFF, 0xFF, 255]);
    px(img, 42 + ox, 4, [0xFF, 0xFF, 0xFF, 200]);
    px(img, 46 + ox, 2, [0xFF, 0xFF, 0xFF, 150]);
}

fn draw_slime_king_celebrate(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let wobble = (phase * PI * 8.0).abs().sin() * 6.0;
    draw_slime_king_body(img, ox, wobble);
    px(img, 30 + ox, 2, [0xFF, 0xFF, 0x00, 255]);
}

fn draw_slime_king_body(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, wobble: f32) {
    let wob_y = wobble as i32;

    for y in 20..52_i32 {
        for x in 12..48_i32 {
            let dx = x as f32 - 30.0;
            let dy = y as f32 - 36.0 + wobble;
            let d = (dx * dx) / 256.0 + (dy * dy) / 144.0;
            if d < 1.0 {
                let c = if d > 0.85 {
                    [0x22, 0xCC, 0x22, 255]
                } else {
                    [0x44, 0xFF, 0x44, 255]
                };
                px(img, x + ox, y + wob_y, c);
            }
        }
    }

    for y in 6..14_i32 {
        for x in 22..38_i32 {
            let dx = x as f32 - 30.0;
            let dy = y as f32 - 10.0;
            let d = (dx * dx) / 36.0 + (dy * dy) / 16.0;
            if d < 1.0 {
                px(img, x + ox, y + wob_y, [0xFF, 0xFF, 0x00, 255]);
            }
        }
    }

    px(img, 26 + ox, 28 + wob_y, [0xFF, 0xFF, 0xFF, 255]);
    px(img, 34 + ox, 28 + wob_y, [0xFF, 0xFF, 0xFF, 255]);
    px(img, 27 + ox, 28 + wob_y, [0x00, 0x00, 0x00, 255]);
    px(img, 33 + ox, 28 + wob_y, [0x00, 0x00, 0x00, 255]);
    px(img, 26 + ox, 36 + wob_y, [0x00, 0x00, 0x00, 255]);
    px(img, 30 + ox, 38 + wob_y, [0x00, 0x00, 0x00, 255]);
    px(img, 34 + ox, 36 + wob_y, [0x00, 0x00, 0x00, 255]);
}

// ============================================================================
// PIXEL WIZARD
// ============================================================================

fn draw_pixel_wizard_idle(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let bob = (phase * PI * 2.0).sin() * 1.0;
    draw_pixel_wizard_body(img, ox, bob, 0);
}

fn draw_pixel_wizard_walk(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let bob = (phase * PI * 4.0).sin() * 2.0;
    let staff_bounce = (phase * PI * 4.0).cos() as i32 * 2;
    draw_pixel_wizard_body(img, ox, bob, staff_bounce);
}

fn draw_pixel_wizard_fly(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let bob = (phase * PI * 6.0).sin() * 4.0;
    draw_pixel_wizard_body(img, ox, bob, 0);
    for i in 0..8 {
        let angle = i as f32 * PI / 4.0;
        let dist = 35.0 + (phase * PI * 4.0).sin() * 5.0;
        px(
            img,
            30 + (angle.cos() * dist) as i32 + ox,
            30 + (angle.sin() * dist) as i32,
            [0xFF, 0x00, 0xFF, 150],
        );
    }
}

fn draw_pixel_wizard_sleep(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let breath = (phase * PI * 1.0).sin() * 0.5;
    draw_pixel_wizard_body(img, ox, breath, 0);
    px(img, 30 + ox, 24, [0x88, 0x88, 0xFF, 200]);
}

fn draw_pixel_wizard_celebrate(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let jump = (phase * PI * 8.0).abs().sin() * 5.0;
    draw_pixel_wizard_body(img, ox, -jump, 0);
    for i in 0..6 {
        px(
            img,
            ox + 10 + i * 10,
            10 + (i % 3) * 8,
            [0xFF, 0xFF, 0x00, 255],
        );
    }
}

fn draw_pixel_wizard_body(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    ox: i32,
    bob: f32,
    staff_bounce: i32,
) {
    let bob_y = bob as i32;

    for y in 24..52_i32 {
        for x in 16..44_i32 {
            let dx = x as f32 - 30.0;
            let dy = y as f32 - 38.0 + bob;
            let d = (dx * dx) / 144.0 + (dy * dy) / 100.0;
            if d < 1.0 {
                px(img, x + ox, y + bob_y, [0x88, 0x00, 0xFF, 255]);
            }
        }
    }

    for y in 10..24_i32 {
        for x in 22..38_i32 {
            let dx = x as f32 - 30.0;
            let dy = y as f32 - 17.0 + bob;
            let d = (dx * dx) / 36.0 + (dy * dy) / 36.0;
            if d < 1.0 {
                px(img, x + ox, y + bob_y, [0xFF, 0xCC, 0x99, 255]);
            }
        }
    }

    for y in 0..16_i32 {
        for x in 20..40_i32 {
            let dx = x as f32 - 30.0;
            let dy = y as f32 - 8.0;
            let d = (dx * dx) / 25.0 + (dy * dy) / 64.0;
            if d < 1.0 {
                px(img, x + ox, y + bob_y, [0xFF, 0x00, 0xFF, 255]);
            }
        }
    }

    px(img, 26 + ox, 17 + bob_y, [0xFF, 0xFF, 0xFF, 255]);
    px(img, 34 + ox, 17 + bob_y, [0xFF, 0xFF, 0xFF, 255]);
    px(img, 27 + ox, 17 + bob_y, [0x00, 0x00, 0x00, 255]);
    px(img, 33 + ox, 17 + bob_y, [0x00, 0x00, 0x00, 255]);

    for y in 22..32_i32 {
        for x in 24..36_i32 {
            let dx = x as f32 - 30.0;
            let dy = y as f32 - 27.0;
            let d = (dx * dx) / 16.0 + (dy * dy) / 25.0;
            if d < 1.0 {
                px(img, x + ox, y + bob_y, [0xFF, 0xFF, 0xFF, 255]);
            }
        }
    }

    for y in 20..52_i32 {
        for x in 46..52_i32 {
            px(
                img,
                x + ox,
                y + bob_y + staff_bounce,
                [0x8B, 0x45, 0x13, 255],
            );
        }
    }
    for y in 12..20_i32 {
        for x in 44..54_i32 {
            let dx = x as f32 - 49.0;
            let dy = y as f32 - 16.0;
            let d = (dx * dx) / 9.0 + (dy * dy) / 9.0;
            if d < 1.0 {
                px(
                    img,
                    x + ox,
                    y + bob_y + staff_bounce,
                    [0xFF, 0xFF, 0x00, 255],
                );
            }
        }
    }
}

// ============================================================================

// Ghost
fn draw_ghost_idle(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let bob = (phase * PI * 2.0).sin() * 2.0;
    for dy in 0i32..64i32 {
        for dx in 0i32..64i32 {
            let px = ox + dx;
            let py = 4 + dy + bob as i32;
            if px >= 0 && px < img.width() as i32 && py >= 0 && py < img.height() as i32 {
                let cx = 32.0;
                let cy = 28.0;
                let dist = ((dx as f32 - cx).powi(2) + (dy as f32 - cy).powi(2)).sqrt();
                if dist < 24.0 {
                    let alpha = (200.0 - dist * 7.0) as u8;
                    img.put_pixel(px as u32, py as u32, Rgba([220, 220, 255, alpha]));
                }
                // Eyes
                if (20..=28).contains(&dy) && ((18..=26).contains(&dx) || (38..=46).contains(&dx)) {
                    img.put_pixel(px as u32, py as u32, Rgba([0, 0, 0, 255]));
                }
            }
        }
    }
}

fn draw_ghost_walk(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    draw_ghost_idle(img, ox, phase);
    let wave = (phase * PI * 2.0).sin() * 3.0;
    for dx in 0i32..64i32 {
        let py = 60 + (dx as f32 * 0.3).sin() as i32 + wave as i32;
        if ox + dx >= 0 && ox + dx < img.width() as i32 && py >= 0 && py < img.height() as i32 {
            img.put_pixel((ox + dx) as u32, py as u32, Rgba([200, 200, 240, 150]));
        }
    }
}

fn draw_ghost_fly(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    draw_ghost_idle(img, ox, phase);
    for i in 0i32..5i32 {
        for dx in 0i32..16i32 {
            let px = ox + dx;
            let py = 64 + i * 8 + (dx as f32 * 0.3).sin() as i32;
            if px >= 0 && px < img.width() as i32 && py >= 0 && py < img.height() as i32 {
                img.put_pixel(
                    px as u32,
                    py as u32,
                    Rgba([200, 200, 240, (100 - i * 15) as u8]),
                );
            }
        }
    }
}

fn draw_ghost_sleep(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    draw_ghost_idle(img, ox, phase);
    // Z's floating up
    for i in 0i32..3i32 {
        let z_x = ox + 40 + i * 12;
        let z_y = 10 - i * 8 + (phase * PI * 2.0).sin() as i32;
        for dx in 0i32..8i32 {
            if z_x + dx >= 0
                && z_x + dx < img.width() as i32
                && z_y >= 0
                && z_y < img.height() as i32
            {
                img.put_pixel((z_x + dx) as u32, z_y as u32, Rgba([255, 255, 255, 200]));
            }
        }
    }
}

fn draw_ghost_celebrate(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    draw_ghost_idle(img, ox, phase);
    // Sparkles around
    for i in 0i32..8i32 {
        let angle = i as f32 * PI / 4.0 + phase * PI * 2.0;
        for r in 24i32..36i32 {
            let px = ox + 32 + (angle.cos() * r as f32) as i32;
            let py = 32 + (angle.sin() * r as f32) as i32;
            if px >= 0 && px < img.width() as i32 && py >= 0 && py < img.height() as i32 {
                img.put_pixel(px as u32, py as u32, Rgba([255, 255, 100, 200]));
            }
        }
    }
}

// COSMIC JELLYFISH
// ============================================================================

fn draw_cosmic_jellyfish_idle(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let pulse = (phase * PI * 2.0).sin() * 1.5;
    draw_cosmic_jellyfish_body(img, ox, pulse);
}

fn draw_cosmic_jellyfish_walk(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let pulse = (phase * PI * 4.0).sin() * 2.5;
    draw_cosmic_jellyfish_body(img, ox, pulse);
}

fn draw_cosmic_jellyfish_fly(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let pulse = (phase * PI * 6.0).sin() * 4.0;
    draw_cosmic_jellyfish_body(img, ox, pulse);
    for i in 0..5_i32 {
        px(
            img,
            ox + 10 + i * 8,
            50 + i * 3,
            [0xFF, 0xFF, 0xFF, (200 - i * 30) as u8],
        );
    }
}

fn draw_cosmic_jellyfish_sleep(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let pulse = (phase * PI * 1.0).sin() * 0.5;
    draw_cosmic_jellyfish_body(img, ox, pulse);
    px(img, 38 + ox, 4, [0xFF, 0xFF, 0xFF, 255]);
    px(img, 42 + ox, 2, [0xFF, 0xFF, 0xFF, 200]);
}

fn draw_cosmic_jellyfish_celebrate(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, phase: f32) {
    let pulse = (phase * PI * 8.0).abs().sin() * 6.0;
    draw_cosmic_jellyfish_body(img, ox, pulse);
    for i in 0..12_i32 {
        let angle = i as f32 * PI / 6.0;
        let dist = 25.0 + (phase * PI * 4.0).sin() * 10.0;
        px(
            img,
            30 + (angle.cos() * dist) as i32 + ox,
            30 + (angle.sin() * dist) as i32,
            [0xFF, 0xFF, 0x00, 200],
        );
    }
}

fn draw_cosmic_jellyfish_body(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, ox: i32, pulse: f32) {
    let pul_y = pulse as i32;

    for y in 16..36_i32 {
        for x in 12..48_i32 {
            let dx = x as f32 - 30.0;
            let dy = y as f32 - 26.0 + pulse;
            let d = (dx * dx) / 225.0 + (dy * dy) / 100.0;
            if d < 1.0 {
                let c = if d > 0.85 {
                    [0x88, 0x44, 0xFF, 255]
                } else {
                    [0xFF, 0x44, 0xFF, 200]
                };
                px(img, x + ox, y + pul_y, c);
            }
        }
    }

    for y in 36..60_i32 {
        for x in 16..24_i32 {
            px(img, x + ox, y + pul_y, [0x44, 0xFF, 0xFF, 180]);
        }
        for x in 28..36_i32 {
            px(img, x + ox, y + pul_y, [0x44, 0xFF, 0xFF, 180]);
        }
        for x in 38..46_i32 {
            px(img, x + ox, y + pul_y, [0x44, 0xFF, 0xFF, 180]);
        }
    }

    px(img, 26 + ox, 22 + pul_y, [0xFF, 0xFF, 0xFF, 255]);
    px(img, 34 + ox, 22 + pul_y, [0xFF, 0xFF, 0xFF, 255]);
    px(img, 27 + ox, 22 + pul_y, [0x00, 0x00, 0x00, 255]);
    px(img, 33 + ox, 22 + pul_y, [0x00, 0x00, 0x00, 255]);

    for i in 0..5_i32 {
        px(
            img,
            ox + 10 + i * 8,
            8 + (i % 3) * 6,
            [0xFF, 0xFF, 0xFF, 255],
        );
    }
}

// ============================================================================
// MAIN
// ============================================================================

/// A single sprite frame buffer, ready to draw.
type FrameCanvas = ImageBuffer<Rgba<u8>, Vec<u8>>;

/// Draws one frame at index `i` into a `SZ * SC` square canvas.
type FrameDrawFn = fn(&mut FrameCanvas, i32, f32);

fn generate_sprite_sheet(_name: &str, draw_fn: FrameDrawFn, output_path: &str) {
    let frames = 4;
    let w = (SZ * SC * frames) as i32;
    let h = (SZ * SC) as i32;

    let mut sheet = ImageBuffer::from_pixel(w as u32, h as u32, Rgba([0, 0, 0, 0]));
    for f in 0..frames {
        let ox = (f as i32) * SZ as i32;
        let phase = f as f32 / frames as f32;
        draw_fn(&mut sheet, ox, phase);
    }
    sheet.save(output_path).unwrap();
    println!("Generated {}", output_path);
}

fn main() {
    std::fs::create_dir_all("assets/generated").unwrap();

    generate_sprite_sheet(
        "bird_idle",
        draw_bird_idle,
        "assets/generated/bird_idle.png",
    );
    generate_sprite_sheet(
        "bird_walk",
        draw_bird_walk,
        "assets/generated/bird_walk.png",
    );
    generate_sprite_sheet("bird_fly", draw_bird_fly, "assets/generated/bird_fly.png");
    generate_sprite_sheet(
        "bird_sleep",
        draw_bird_sleep,
        "assets/generated/bird_sleep.png",
    );
    generate_sprite_sheet(
        "bird_celebrate",
        draw_bird_celebrate,
        "assets/generated/bird_celebrate.png",
    );

    generate_sprite_sheet(
        "robo_cat_idle",
        draw_robo_cat_idle,
        "assets/generated/robo_cat_idle.png",
    );
    generate_sprite_sheet(
        "robo_cat_walk",
        draw_robo_cat_walk,
        "assets/generated/robo_cat_walk.png",
    );
    generate_sprite_sheet(
        "robo_cat_fly",
        draw_robo_cat_fly,
        "assets/generated/robo_cat_fly.png",
    );
    generate_sprite_sheet(
        "robo_cat_sleep",
        draw_robo_cat_sleep,
        "assets/generated/robo_cat_sleep.png",
    );
    generate_sprite_sheet(
        "robo_cat_celebrate",
        draw_robo_cat_celebrate,
        "assets/generated/robo_cat_celebrate.png",
    );

    generate_sprite_sheet(
        "slime_king_idle",
        draw_slime_king_idle,
        "assets/generated/slime_king_idle.png",
    );
    generate_sprite_sheet(
        "slime_king_walk",
        draw_slime_king_walk,
        "assets/generated/slime_king_walk.png",
    );
    generate_sprite_sheet(
        "slime_king_fly",
        draw_slime_king_fly,
        "assets/generated/slime_king_fly.png",
    );
    generate_sprite_sheet(
        "slime_king_sleep",
        draw_slime_king_sleep,
        "assets/generated/slime_king_sleep.png",
    );
    generate_sprite_sheet(
        "slime_king_celebrate",
        draw_slime_king_celebrate,
        "assets/generated/slime_king_celebrate.png",
    );

    generate_sprite_sheet(
        "pixel_wizard_idle",
        draw_pixel_wizard_idle,
        "assets/generated/pixel_wizard_idle.png",
    );
    generate_sprite_sheet(
        "pixel_wizard_walk",
        draw_pixel_wizard_walk,
        "assets/generated/pixel_wizard_walk.png",
    );
    generate_sprite_sheet(
        "pixel_wizard_fly",
        draw_pixel_wizard_fly,
        "assets/generated/pixel_wizard_fly.png",
    );
    generate_sprite_sheet(
        "pixel_wizard_sleep",
        draw_pixel_wizard_sleep,
        "assets/generated/pixel_wizard_sleep.png",
    );
    generate_sprite_sheet(
        "pixel_wizard_celebrate",
        draw_pixel_wizard_celebrate,
        "assets/generated/pixel_wizard_celebrate.png",
    );

    generate_sprite_sheet(
        "ghost_idle",
        draw_ghost_idle,
        "assets/generated/ghost_idle.png",
    );
    generate_sprite_sheet(
        "ghost_walk",
        draw_ghost_walk,
        "assets/generated/ghost_walk.png",
    );
    generate_sprite_sheet(
        "ghost_fly",
        draw_ghost_fly,
        "assets/generated/ghost_fly.png",
    );
    generate_sprite_sheet(
        "ghost_sleep",
        draw_ghost_sleep,
        "assets/generated/ghost_sleep.png",
    );
    generate_sprite_sheet(
        "ghost_celebrate",
        draw_ghost_celebrate,
        "assets/generated/ghost_celebrate.png",
    );
    generate_sprite_sheet(
        "cosmic_jellyfish_idle",
        draw_cosmic_jellyfish_idle,
        "assets/generated/cosmic_jellyfish_idle.png",
    );
    generate_sprite_sheet(
        "cosmic_jellyfish_walk",
        draw_cosmic_jellyfish_walk,
        "assets/generated/cosmic_jellyfish_walk.png",
    );
    generate_sprite_sheet(
        "cosmic_jellyfish_fly",
        draw_cosmic_jellyfish_fly,
        "assets/generated/cosmic_jellyfish_fly.png",
    );
    generate_sprite_sheet(
        "cosmic_jellyfish_sleep",
        draw_cosmic_jellyfish_sleep,
        "assets/generated/cosmic_jellyfish_sleep.png",
    );
    generate_sprite_sheet(
        "cosmic_jellyfish_celebrate",
        draw_cosmic_jellyfish_celebrate,
        "assets/generated/cosmic_jellyfish_celebrate.png",
    );

    // Dragon animations
    generate_sprite_sheet(
        "dragon_idle",
        draw_dragon_idle,
        "assets/generated/dragon_idle.png",
    );
    generate_sprite_sheet(
        "dragon_walk",
        draw_dragon_walk,
        "assets/generated/dragon_walk.png",
    );
    generate_sprite_sheet(
        "dragon_fly",
        draw_dragon_fly,
        "assets/generated/dragon_fly.png",
    );
    generate_sprite_sheet(
        "dragon_sleep",
        draw_dragon_sleep,
        "assets/generated/dragon_sleep.png",
    );
    generate_sprite_sheet(
        "dragon_celebrate",
        draw_dragon_celebrate,
        "assets/generated/dragon_celebrate.png",
    );

    println!("\nAll sprite sheets generated! (30 total)");
}
