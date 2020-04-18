use crate::{Error, throws, vector::size2, image::{Image, bgra8}};

pub type Target<'t> = Image<&'t mut[bgra8]>;
pub trait Widget {
    fn size(&mut self, size : size2) -> size2 { size }
    fn render(&mut self, target : &mut Target);
}

#[throws]
pub fn window<'t>(widget: &'t mut dyn Widget) {
    use smithay_client_toolkit::{
        default_environment, environment::SimpleGlobal, init_default_environment,
        reexports::calloop::{EventLoop, LoopSignal}, WaylandSource,
        get_surface_scale_factor, shm,
        seat::keyboard::{self, map_keyboard, RepeatKind},
        reexports::{
            client::protocol::{wl_surface::WlSurface as Surface, wl_pointer as pointer},
            protocols::wlr::unstable::layer_shell::v1::client::{
                zwlr_layer_shell_v1::{self as layer_shell, ZwlrLayerShellV1 as LayerShell},
                zwlr_layer_surface_v1 as layer_surface
            },
        },
    };

    default_environment!(Compositor,
        fields = [ layer_shell: SimpleGlobal<LayerShell> ],
        singles = [ LayerShell => layer_shell ]
    );

    let (compositor, display, queue) = init_default_environment!(Compositor, fields = [layer_shell: SimpleGlobal::new()])?;

    struct State<'t> { signal: LoopSignal, pool: shm::MemPool, widget: &'t mut dyn Widget, unscaled_size: size2 }

    fn draw(pool: &mut shm::MemPool, surface: &Surface, widget: &mut dyn Widget, size: size2) {
        let stride = size.x*4;
        pool.resize((size.y*stride) as usize).unwrap();
        let buffer = pool.buffer(0, size.x as i32, size.y as i32, stride as i32, shm::Format::Argb8888);
        let mut target = Target::from_bytes(pool.mmap(), size);
        widget.render(&mut target);
        surface.attach(Some(&buffer), 0, 0);
        //surface.damage_buffer(0, 0, size.x as i32, size.y as i32);
        surface.commit();
    }

    let surface = compositor.create_surface_with_scale_callback(|scale, surface, mut state| {
        let State{ pool, widget, unscaled_size, .. } = state.get().unwrap();
        surface.set_buffer_scale(scale);
        println!("{:?} {:?}", smithay_client_toolkit::get_surface_outputs(&surface).len(), scale);
        draw(pool, &surface, *widget, (scale as u32)* *unscaled_size);
    });
    /*surface.quick_assign(|_, event, mut state| {
        match event {
            smithay_client_toolkit::reexports::client::protocol::wl_surface::Event::Enter{output} =>
                panic!("{:?}", smithay_client_toolkit::output::with_output_info(&output, |info| info.scale_factor).unwrap()),
            _ => panic!("{:?}", event),
        }
    });*/

    let layer_shell = compositor.require_global::<LayerShell>();
    let layer_surface = layer_shell.get_layer_surface(&surface, None, layer_shell::Layer::Overlay, "framework".to_string());
    layer_surface.set_keyboard_interactivity(1);

    let mut event_loop = EventLoop::<State>::new()?;
    event_loop.handle().insert_source(WaylandSource::new(queue), |e, _| { e.unwrap(); } ).unwrap();

    surface.commit();
    layer_surface.quick_assign(/*surface*/ move |layer_surface, event, mut state| {
        let State{ signal, pool, widget, ref mut unscaled_size, ..} = state.get().unwrap();
        match event {
            layer_surface::Event::Closed => signal.stop(),
            layer_surface::Event::Configure{serial, width, height} => {
                let scale = get_surface_scale_factor(&surface) as u32;
                println!("{:?} {:?}", smithay_client_toolkit::get_surface_outputs(&surface).len(), scale);
                if !(width > 0 && height > 0) {
                    let size = widget.size(size2{x:3840, y:2160}); // FIXME: get output size
                    layer_surface.set_size(size.x/scale, size.y/scale);
                    layer_surface.ack_configure(serial);
                    surface.commit();
                    return;
                }
                layer_surface.ack_configure(serial);
                *unscaled_size = size2{x:width, y:height};
                draw(pool, &surface, *widget, scale* *unscaled_size);
            }
            _ => unimplemented!(),
        }
    });

    for seat in compositor.get_all_seats() {
        let (_, repeat_source) = map_keyboard(&seat, None, RepeatKind::System, move |event, _, mut state| {
            let State{signal, /*ref mut widget,*/..} = state.get().unwrap();
            match event {
                keyboard::Event::Enter { /*keysyms,*/ .. } => {},
                keyboard::Event::Leave { .. } => {}
                keyboard::Event::Key { keysym, state, utf8, .. } => { println!("{:?}: {:x} '{:?}'", state, keysym, utf8); signal.stop(); }
                keyboard::Event::Modifiers { /*modifiers*/.. } => {},
                keyboard::Event::Repeat { keysym, utf8, .. } => { println!("{:x} '{:?}'", keysym, utf8); },
            }
        }).unwrap();
        event_loop.handle().insert_source(repeat_source, |_, _| {}).unwrap();
        seat.get_pointer().quick_assign(|_, event, mut state| {
            let State{signal,/*ref mut widget,*/.. } = state.get().unwrap();
            match event {
                pointer::Event::Leave{..} => signal.stop(),
                pointer::Event::Motion{/*surface_x, surface_y,*/..} => {},
                pointer::Event::Button{/*button, state,*/..} => {},
                _ => {},
            }
        });
    }

    let mut state = State::</*'t*/'_>{
        signal: event_loop.get_signal(),
        pool: compositor.create_simple_pool(|_|())?,
        widget: unsafe{std::mem::transmute::<&mut dyn Widget, &'static mut dyn Widget>(widget)},
        unscaled_size: size2{x:0,y:0}
    };
    display.flush()?;
    event_loop.run(None, &mut state, |_| display.flush().unwrap())? // "`widget` escapes the function body here": How ?
}
