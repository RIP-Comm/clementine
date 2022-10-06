use std::{
    env, error, fs,
    io::Read,
    sync::{Arc, Mutex},
};

use emu_clementine::cartridge_header::CartridgeHeader;
use glium::glutin;

use emu_clementine::{arm7tdmi::Arm7tdmi, cpu::Cpu};
use ui_clementine::egui_app::EguiApp;

fn main() {
    println!("clementine v0.1.0");

    let cartridge_name = env::args().skip(1).collect::<Vec<String>>();

    let name = match cartridge_name.first() {
        Some(name) => {
            println!("loading {name}");
            name
        }
        None => {
            println!("no cartridge found :(");
            std::process::exit(1)
        }
    };

    let data = match read_file(name) {
        Ok(d) => d,
        Err(e) => {
            println!("{e}");
            std::process::exit(2);
        }
    };

    let cartridge_header = CartridgeHeader::new(&data).unwrap();
    println!("{}", cartridge_header.game_title);

    let cpu = Arm7tdmi::new(data);

    let app = Arc::new(Mutex::new(EguiApp::new(cpu)));

    let event_loop = glutin::event_loop::EventLoop::new();
    let window_builder = glutin::window::WindowBuilder::new()
        .with_resizable(true)
        .with_inner_size(glutin::dpi::LogicalSize {
            width: 1280.0,
            height: 720.0,
        })
        .with_title("clementine v0.1.0");

    let context_builder = glutin::ContextBuilder::new()
        .with_depth_buffer(0)
        .with_srgb(true)
        .with_stencil_buffer(0)
        .with_vsync(true);

    let display = glium::Display::new(window_builder, context_builder, &event_loop).unwrap();
    let mut egui_glium = egui_glium::EguiGlium::new(&display, &event_loop);

    let application = Arc::clone(&app);
    std::thread::spawn(move || loop {
        match application.lock() {
            Ok(mut a) => a.gba.cpu.step(),
            Err(_) => continue,
        }
    });

    event_loop.run(move |event, _, control_flow| {
        display.gl_window().window().request_redraw();

        match event {
            glutin::event::Event::RedrawRequested(_) => {
                egui_glium.run(&display, |egui_ctx| {
                    let mut a = app.lock().expect("lock app mutex");
                    a.draw(egui_ctx);
                });

                display.gl_window().window().request_redraw();
                let mut target = display.draw();
                egui_glium.paint(&display, &mut target);
                target.finish().unwrap();
            }
            glutin::event::Event::WindowEvent { event, .. } => {
                if matches!(
                    event,
                    glutin::event::WindowEvent::CloseRequested
                        | glutin::event::WindowEvent::Destroyed
                ) {
                    *control_flow = glutin::event_loop::ControlFlow::Exit;
                }

                display.gl_window().window().request_redraw();
            }
            glutin::event::Event::NewEvents(glutin::event::StartCause::ResumeTimeReached {
                ..
            }) => {
                display.gl_window().window().request_redraw();
            }
            _ => (),
        }
    });
}

fn read_file(filepath: &str) -> Result<Vec<u8>, Box<dyn error::Error>> {
    let mut f = fs::File::open(filepath)?;
    let mut buf = vec![];
    f.read_to_end(&mut buf)?;

    Ok(buf)
}
