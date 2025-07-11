use std::{env, process};

use libwayshot::WayshotConnection;
use raylib::{
    core::math::Vector2,
    ffi::{Image as FfiImage, SetWindowMonitor, ToggleFullscreen},
    prelude::*,
};
const SPOTLIGHT_TINT: Color = Color::new(0x00, 0x00, 0x00, 190);

fn main() {
    let mut args = env::args();
    let bin = args.next().unwrap();

    let mut monitor_name: Option<String> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--monitor" => {
                monitor_name = args.next().or_else(|| {
                    eprintln!("--monitor needs a value");
                    process::exit(1);
                })
            }
            _other => print_help_and_exit(&bin),
        }
    }

    let wayshot_connection = WayshotConnection::new().expect("Failed to connect to wayshot");
    let outputs = wayshot_connection.get_all_outputs();

    if outputs.is_empty() {
        eprintln!("No Wayland outputs found.");
        process::exit(1);
    }

    let selected_output = match monitor_name {
        None => &outputs[0],
        Some(ref name) => outputs
            .iter()
            .find(|out| &out.name == name)
            .unwrap_or_else(|| {
                eprintln!("Output '{}' not found.", name);
                process::exit(1);
            }),
    };

    let screenshot_image = wayshot_connection
        .screenshot_all(false)
        .expect("failed to take a screenshot")
        .to_rgba8();
    let (width, height) = screenshot_image.dimensions();
    let (mut rl, thread) = raylib::init()
        .title(env!("CARGO_BIN_NAME"))
        .size(
            selected_output
                .logical_region
                .inner
                .size
                .width
                .try_into()
                .unwrap(),
            selected_output
                .logical_region
                .inner
                .size
                .height
                .try_into()
                .unwrap(),
        )
        .transparent()
        .undecorated()
        .vsync()
        .build();

    let idx = outputs
        .iter()
        .position(|o| o.name == selected_output.name)
        .expect("Monitor not found");

    unsafe {
        ToggleFullscreen();
    }

    unsafe {
        SetWindowMonitor(idx as i32);
    }

    let screenshot_image = unsafe {
        Image::from_raw(FfiImage {
            // We can leak memory here because raylib will free the memory for us
            data: Box::new(screenshot_image.into_vec())
                .leak()
                .as_mut_ptr()
                .cast(),
            format: PixelFormat::PIXELFORMAT_UNCOMPRESSED_R8G8B8A8 as i32,
            mipmaps: 1,
            width: width as i32,
            height: height as i32,
        })
    };
    let screenshot_texture = rl
        .load_texture_from_image(&thread, &screenshot_image)
        .expect("failed to load screenshot into a texture");
    #[cfg(feature = "dev")]
    let mut spotlight_shader = rl
        .load_shader(&thread, None, Some("shaders/spotlight.fs"))
        .expect("Failed to load spotlight shader");

    #[cfg(not(feature = "dev"))]
    let mut spotlight_shader =
        rl.load_shader_from_memory(&thread, None, Some(include_str!("../shaders/spotlight.fs")));
    let mut rl_camera = Camera2D::default();
    rl_camera.zoom = 1.0;
    rl_camera.target = Vector2::new(
        selected_output.logical_region.inner.position.x as f32,
        selected_output.logical_region.inner.position.y as f32,
    );

    let mut delta_scale = 0f64;
    let mut scale_pivot = rl.get_mouse_position();
    let mut velocity = Vector2::default();
    let mut spotlight_radius_multiplier = 1.0;
    let mut spotlight_radius_multiplier_delta = 0.0;

    #[cfg(feature = "dev")]
    let mut spotlight_tint_uniform_location;
    #[cfg(feature = "dev")]
    let mut cursor_position_uniform_location;
    #[cfg(feature = "dev")]
    let mut spotlight_radius_multiplier_uniform_location;
    #[cfg(not(feature = "dev"))]
    let spotlight_tint_uniform_location;
    #[cfg(not(feature = "dev"))]
    let cursor_position_uniform_location;
    #[cfg(not(feature = "dev"))]
    let spotlight_radius_multiplier_uniform_location;

    spotlight_tint_uniform_location = spotlight_shader.get_shader_location("spotlightTint");
    cursor_position_uniform_location = spotlight_shader.get_shader_location("cursorPosition");
    spotlight_radius_multiplier_uniform_location =
        spotlight_shader.get_shader_location("spotlightRadiusMultiplier");
    while !rl.window_should_close() {
        if rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_RIGHT) {
            break;
        }
        #[cfg(feature = "dev")]
        if rl.is_key_pressed(KeyboardKey::KEY_R) {
            spotlight_shader = rl
                .load_shader(&thread, None, Some("shaders/spotlight.fs"))
                .expect("Failed to load spotlight shader");
            spotlight_tint_uniform_location = spotlight_shader.get_shader_location("spotlightTint");
            cursor_position_uniform_location =
                spotlight_shader.get_shader_location("cursorPosition");
            spotlight_radius_multiplier_uniform_location =
                spotlight_shader.get_shader_location("spotlightRadiusMultiplier");
        }
        let enable_spotlight = rl.is_key_down(KeyboardKey::KEY_LEFT_CONTROL)
            || rl.is_key_down(KeyboardKey::KEY_RIGHT_CONTROL);
        let mut scrolled_amount = rl.get_mouse_wheel_move_v().y;
        if rl.is_key_down(KeyboardKey::KEY_U) {
            scrolled_amount += 0.1;
        }
        if rl.is_key_down(KeyboardKey::KEY_D) {
            scrolled_amount -= 0.1;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_LEFT_CONTROL)
            || rl.is_key_pressed(KeyboardKey::KEY_RIGHT_CONTROL)
        {
            spotlight_radius_multiplier = 5.0;
            spotlight_radius_multiplier_delta = -15.0;
        }
        if scrolled_amount != 0.0 {
            match (
                enable_spotlight,
                rl.is_key_down(KeyboardKey::KEY_LEFT_SHIFT)
                    || rl.is_key_down(KeyboardKey::KEY_RIGHT_SHIFT),
            ) {
                (_, false) => {
                    delta_scale += scrolled_amount as f64;
                }
                (true, true) => {
                    spotlight_radius_multiplier_delta -= scrolled_amount as f64;
                }
                _ => {}
            }
            scale_pivot =
                if rl.is_key_down(KeyboardKey::KEY_U) || rl.is_key_down(KeyboardKey::KEY_D) {
                    Vector2 {
                        x: (rl.get_screen_width() / 2) as f32,
                        y: (rl.get_screen_height() / 2) as f32,
                    }
                } else {
                    rl.get_mouse_position()
                }
        }
        if delta_scale.abs() > 0.5 {
            let p0 = scale_pivot / rl_camera.zoom;
            rl_camera.zoom = (rl_camera.zoom as f64 + delta_scale * rl.get_frame_time() as f64)
                .clamp(1.0, 10.) as f32;
            let p1 = scale_pivot / rl_camera.zoom;
            rl_camera.target += p0 - p1;
            delta_scale -= delta_scale * rl.get_frame_time() as f64 * 4.0
        }
        spotlight_radius_multiplier = (spotlight_radius_multiplier as f64
            + spotlight_radius_multiplier_delta * rl.get_frame_time() as f64)
            .clamp(0.3, 10.) as f32;
        spotlight_radius_multiplier_delta -=
            spotlight_radius_multiplier_delta * rl.get_frame_time() as f64 * 4.0;
        const VELOCITY_THRESHOLD: f32 = 15.0;

        {
            let mut delta = Vector2 { x: 0.0, y: 0.0 };
            for (key, dx, dy) in [
                (KeyboardKey::KEY_H, VELOCITY_THRESHOLD, 0.0),
                (KeyboardKey::KEY_J, 0.0, -VELOCITY_THRESHOLD),
                (KeyboardKey::KEY_K, 0.0, VELOCITY_THRESHOLD),
                (KeyboardKey::KEY_L, -VELOCITY_THRESHOLD, 0.0),
            ] {
                if rl.is_key_down(key) {
                    delta += rl.get_screen_to_world2D(
                        rl.get_mouse_position() - (raylib::core::math::Vector2 { x: dx, y: dy }),
                        rl_camera,
                    ) - rl.get_screen_to_world2D(rl.get_mouse_position(), rl_camera);
                }
            }
            rl_camera.target += delta;
        }
        if rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT) {
            let delta = rl
                .get_screen_to_world2D(rl.get_mouse_position() - rl.get_mouse_delta(), rl_camera)
                - rl.get_screen_to_world2D(rl.get_mouse_position(), rl_camera);
            rl_camera.target += delta;
            velocity = delta * rl.get_fps().as_f32();
        } else if velocity.length_sqr() > VELOCITY_THRESHOLD * VELOCITY_THRESHOLD {
            rl_camera.target += velocity * rl.get_frame_time();
            velocity -= velocity * rl.get_frame_time() * 6.0;
        }

        let mut d = rl.begin_drawing(&thread);
        let mut mode2d = d.begin_mode2D(rl_camera);
        if enable_spotlight {
            mode2d.clear_background(SPOTLIGHT_TINT);
            let mouse_position = mode2d.get_mouse_position();
            spotlight_shader.set_shader_value(
                spotlight_tint_uniform_location,
                SPOTLIGHT_TINT.color_normalize(),
            );
            let screen_height = mode2d.get_screen_height().as_f32();
            spotlight_shader.set_shader_value(
                cursor_position_uniform_location,
                Vector2::new(mouse_position.x, screen_height - mouse_position.y),
            );
            spotlight_shader.set_shader_value(
                spotlight_radius_multiplier_uniform_location,
                spotlight_radius_multiplier,
            );

            let mut shader_mode = mode2d.begin_shader_mode(&mut spotlight_shader);
            shader_mode.draw_texture(&screenshot_texture, 0, 0, Color::WHITE);
        } else {
            mode2d.clear_background(Color::get_color(0));
            mode2d.draw_texture(&screenshot_texture, 0, 0, Color::WHITE);
        }
    }
}

fn print_help_and_exit(bin: &str) -> ! {
    eprintln!(
        "\
{bin}  – Wayland screen-zoom tool

USAGE:
    {bin} [--monitor <name>]

OPTIONS:
    --monitor <name>   Target monitor (Wayland output name); defaults to primary if flag is not provided.",
        bin = bin
    );
    process::exit(0);
}
