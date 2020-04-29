use crate::{Result, widget::Widget};

pub async fn window<W:Widget+'static>(widget: W) -> Result<()> {
    use client_toolkit::{
        default_environment, environment::SimpleGlobal, init_default_environment,
        output::with_output_info, get_surface_scale_factor, shm,
        reexports::{
            client::{self, protocol::{wl_surface::WlSurface as Surface, wl_keyboard as keyboard, wl_pointer as pointer}},
            protocols::wlr::unstable::layer_shell::v1::client::{
                zwlr_layer_shell_v1::{self as layer_shell, ZwlrLayerShellV1 as LayerShell},
                zwlr_layer_surface_v1 as layer_surface
            },
        },
    };
    use crate::{vector::size2, image::bgra8, widget::Target};

    default_environment!(Compositor,
        fields = [ layer_shell: SimpleGlobal<LayerShell> ],
        singles = [ LayerShell => layer_shell ]
    );

    let (env, display, queue) = init_default_environment!(Compositor, fields = [layer_shell: SimpleGlobal::new()])?;

    enum Item {
        Apply(std::io::Result<()>),
        Quit,
    }
    struct State<W:Widget> {
        pool: shm::MemPool,
        widget: W,
        unscaled_size: size2
    }
    struct DispatchData<'t, W:Widget> {
        streams: &'t mut Peekable<SelectAll<LocalBoxStream<'t, Item>>>,
        state: &'t mut State<W>,
    }
    fn quit<Item>(streams: &mut Peekable<SelectAll<LocalBoxStream<'_, Item>>>) { streams.get_mut().push(iter(once(Item::Quit)).boxed_local()) }

    fn draw<W:Widget>(pool: &mut shm::MemPool, surface: &Surface, widget: &mut W, size: size2) {
        let stride = size.x*4;
        pool.resize((size.y*stride) as usize).unwrap();
        let mut target = Target::from_bytes(pool.mmap(), size);
        target.set(|_| bgra8{b:0,g:0,r:0,a:0xFF});
        widget.render(&mut target);
        let buffer = pool.buffer(0, size.x as i32, size.y as i32, stride as i32, shm::Format::Argb8888);
        surface.attach(Some(&buffer), 0, 0);
        surface.damage_buffer(0, 0, size.x as i32, size.y as i32);
        surface.commit();
    }

    let surface = env.create_surface_with_scale_callback(|scale, surface, mut data| {
        let DispatchData{state:State::<W>{pool, widget, unscaled_size, ..}, ..} = data.get().unwrap();
        surface.set_buffer_scale(scale);
        draw(pool, &surface, widget, (scale as u32)* *unscaled_size);
    });

    let layer_shell = env.require_global::<LayerShell>();
    let layer_surface = layer_shell.get_layer_surface(&surface, None, layer_shell::Layer::Overlay, "framework".to_string());
    //layer_surface.set_keyboard_interactivity(1);

    //let mut event_loop = EventLoop::<State>::new()?;
    //event_loop.handle().insert_source(WaylandSource::new(queue), |e, _| { e.unwrap(); } )?;

    surface.commit();
    layer_surface.quick_assign({let env = env.clone(); /*surface*/ move |layer_surface, event, mut data| {
        let DispatchData{streams, state:State::<W>{ pool, widget, ref mut unscaled_size, ..}} = data.get().unwrap();
        use layer_surface::Event::*;
        match event {
            Closed => quit(streams),
            Configure{serial, width, height} => {
                if !(width > 0 && height > 0) {
                    let (scale, size) = with_output_info(
                        env.get_all_outputs().first().unwrap(),
                        |info| (info.scale_factor as u32, info.modes.first().unwrap().dimensions)
                    ).unwrap();
                    let size = widget.size(size2{x:(size.0 as u32), y:(size.1 as u32)});
                    layer_surface.set_size(size.x/scale, size.y/scale);
                    layer_surface.ack_configure(serial);
                    surface.commit();
                    return;
                }
                layer_surface.ack_configure(serial);
                *unscaled_size = size2{x:width, y:height};
                draw(pool, &surface, widget, (get_surface_scale_factor(&surface) as u32) * *unscaled_size);
            }
            _ => unimplemented!(),
        }
    }});

    for seat in env.get_all_seats() { // fixme: use env.listen_for_seats instead
        seat.get_keyboard().quick_assign(move |_, event, mut data| {
            let DispatchData{streams, state:State::<W>{..}} = data.get().unwrap();
            use keyboard::Event::*;
            match event {
                Enter { /*keysyms,*/ .. } => {},
                Leave { .. } => {}
                Key { key, state, .. } => { println!("{:?}: {:x} '{:?}'", state, key, "");  }
                Modifiers { /*modifiers*/.. } => {},
                //Repeat { keysym, utf8, .. } => { println!("{:x} '{:?}'", keysym, utf8); },
            }
        });
        //event_loop.handle().insert_source(repeat_source, |_, _| {}).unwrap();
        seat.get_pointer().quick_assign(|_, event, mut data| {
            let DispatchData{streams, state:State::<W>{..}} = data.get().unwrap();
            match event {
                pointer::Event::Leave{..} => quit(streams),
                pointer::Event::Motion{/*surface_x, surface_y,*/..} => {},
                pointer::Event::Button{/*button, state,*/..} => {},
                _ => {},
            }
        });
    }

    use {std::iter::once, futures::{pin_mut, FutureExt, stream::{unfold, iter, StreamExt, SelectAll, Peekable, LocalBoxStream}}};
    let mut streams = SelectAll::new().peekable();

    // Dispatch socket to per event callbacks which mutate state
    mod nix {
        pub type RawPollFd = std::os::unix::io::RawFd;
        pub trait AsRawPollFd { fn as_raw_poll_fd(&self) -> RawPollFd; }
        impl AsRawPollFd for std::os::unix::io::RawFd { fn as_raw_poll_fd(&self) -> RawPollFd { self.as_raw_fd() } }
    }
    struct Async<T>(T);
    impl<T> Async<T> {
        fn new(poll_fd: impl nix::AsRawPollFd) -> Result<smol::Async<T>, std::io::Error> {
            struct AsRawFd<T>(T);
            impl<T> std::os::unix::io::AsRawFd for AsRawFd<T> { fn as_raw_fd(&self) -> std::os::unix::io::RawFd { self.as_raw_poll_fd() /*->smol::Reactor*/ } }
            smol::Async::new(AsRawFd(poll_fd))
        }
    }
    impl nix::AsRawPollFd for &client::EventQueue {
        fn as_raw_poll_fd(&self) -> nix::RawPollFd { self.display().get_connection_fd() }
    }
    let poll_queue = Async::new(&queue).unwrap();  // Registers in the reactor
    streams.get_mut().push(
        unfold(poll_queue, async move |mut q| {
            Some((Item::Apply(q.with_mut(
                |q:&mut client::EventQueue|
                    q.prepare_read().ok_or(std::io::Error::new(std::io::ErrorKind::Interrupted, "Dispatch all events before polling"))?.read_events()
                ).await), q))
        }).boxed_local()
    );

    let state = State {
        pool: env.create_simple_pool(|_|{})?,
        widget,
        unscaled_size: size2{x:0,y:0}
    };

    loop {
        loop {
            let item = {
                pin_mut!(streams);
                if let Some(item) = streams.peek().now_or_never() { item } else { break; }
            };
            let item = item.ok_or(std::io::Error::new(std::io::ErrorKind::UnexpectedEof,""))?;
            match item {
                Item::Apply(_) => queue.dispatch_pending(&mut DispatchData::<'_>{streams: &mut streams, state: &mut state}, |_,_,_| ()).unwrap(),
                Item::Quit => return Ok(()),
            };
            let _next = streams.next(); // That should just drop the peek
            assert!(_next.now_or_never().is_some());
        }
        display.flush().unwrap();
        pin_mut!(streams);
        streams.peek().await;
    }
    /*let mut state = State::</*'t*/'_>{
        signal: event_loop.get_signal(),
        pool: env.create_simple_pool(|_|{})?,
        widget: unsafe{std::mem::transmute::<&mut dyn Widget, &'static mut dyn Widget>(widget)},
        unscaled_size: size2{x:0,y:0}
    };
    display.flush()?;
    event_loop.run(None, &mut state, |_| display.flush().unwrap())? // "`widget` escapes the function body here": How ?*/
}

pub fn run<'t>(widget: &'t mut dyn Widget) -> Result<()> { smol::run(window(widget)); }
