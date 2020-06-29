use crate::{error::{throws, Error, Result}, widget::Widget};

#[throws]
pub fn window<'w>(widget: &'w mut (dyn Widget + 'w)) -> impl core::future::Future<Output=Result<()>>+'w {
    use client_toolkit::{
        default_environment, environment::SimpleGlobal, init_default_environment,
        seat::{SeatData, with_seat_data}, output::with_output_info, get_surface_outputs, get_surface_scale_factor, shm,
        reexports::{
            client::{self, protocol::{wl_surface::WlSurface as Surface, wl_seat::WlSeat as Seat, wl_keyboard as keyboard, wl_pointer as pointer}},
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

    let (env, _, mut queue) = init_default_environment!(Compositor, fields = [layer_shell: SimpleGlobal::new()])?;

    enum Item {
        Apply(std::io::Result<()>),
        Quit,
    }
    struct State<'w> {
        pool: shm::MemPool,
        widget: &'w mut dyn Widget,
        unscaled_size: size2
    }

    use {std::iter::once, futures::{FutureExt, stream::{unfold, iter, StreamExt, SelectAll, Peekable, LocalBoxStream}}};
    struct DispatchData<'d, 'q, 'w> {
        streams: &'d mut Peekable<SelectAll<LocalBoxStream<'q, Item>>>,
        state: &'d mut State<'w>,
    }
    fn quit(streams: &mut Peekable<SelectAll<LocalBoxStream<'_, Item>>>) { streams.get_mut().push(iter(once(Item::Quit)).boxed_local()) }

    #[throws] fn draw(pool: &mut shm::MemPool, surface: &Surface, widget: &mut dyn Widget, size: size2) {
        let stride = size.x*4;
        pool.resize((size.y*stride) as usize)?;
        let mut target = Target::from_bytes(pool.mmap(), size);
        target.set(|_| bgra8{b:0,g:0,r:0,a:0xFF});
        widget.paint(&mut target)?;
        let buffer = pool.buffer(0, size.x as i32, size.y as i32, stride as i32, shm::Format::Argb8888);
        surface.attach(Some(&buffer), 0, 0);
        surface.damage_buffer(0, 0, size.x as i32, size.y as i32);
        surface.commit();
    }

    // DispatchData wraps using :Any which currently erases lifetimes
    unsafe fn erase_lifetime<'d,'q,'w>(data: DispatchData<'d,'q,'w>) -> DispatchData<'static,'static,'static> {
        std::mem::transmute::<DispatchData::<'d,'q,'w>, DispatchData::<'static,'static,'static>>(data)
    }
    unsafe fn restore_erased_lifetime<'d,'q,'w>(data: &mut DispatchData::<'static,'static,'static>) -> &'d mut DispatchData::<'d,'q,'w> { // fixme: use parent lifetimes
        std::mem::transmute::<&mut DispatchData::<'static,'static,'static>, &mut DispatchData::<'d,'q,'w>>(data)
    }

    let surface = env.create_surface_with_scale_callback(|scale, surface, mut data| {
        let DispatchData{state:State{pool, widget, unscaled_size, ..}, ..} = unsafe{restore_erased_lifetime(data.get().unwrap())};
        surface.set_buffer_scale(scale);
        draw(pool, &surface, *widget, (scale as u32)* *unscaled_size).unwrap()
    });

    let layer_shell = env.require_global::<LayerShell>();
    let layer_surface = layer_shell.get_layer_surface(&surface, None, layer_shell::Layer::Overlay, "framework".to_string());
    layer_surface.set_keyboard_interactivity(1);

    surface.commit();
    layer_surface.quick_assign({let env = env.clone(); /*surface*/ move |layer_surface, event, mut data| {
        let DispatchData{streams, state:State{pool, widget, ref mut unscaled_size, ..}} = unsafe{restore_erased_lifetime(data.get().unwrap())};
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
                let scale = if get_surface_outputs(&surface).is_empty() { // get_surface_outputs defaults to 1 instead of first output factor
					env.get_all_outputs().first().map(|output| with_output_info(output, |info| info.scale_factor)).flatten().unwrap_or(1)
				} else {
					get_surface_scale_factor(&surface)
				};
				surface.set_buffer_scale(scale);
                draw(pool, &surface, *widget, (scale as u32) * *unscaled_size).unwrap();
            }
            _ => unimplemented!(),
        }
    }});

    let handler = move |seat: &client::Attached<Seat>, seat_data: &SeatData| {
        if seat_data.has_keyboard {
            seat.get_keyboard().quick_assign(move |_, event, mut _data| {
                use keyboard::Event::*;
                match event {
                    Keymap {..} => {},
                    Enter { /*keysyms,*/ .. } => {},
                    Leave { .. } => {}
                    Key { key, state, .. } => { todo!("{:?}: {:x}", state, key );  }
                    Modifiers { /*modifiers*/.. } => {},
                    RepeatInfo {..} => {},
                    _ => unreachable!()
                }
            });
        }
        if seat_data.has_pointer {
            seat.get_pointer().quick_assign(|_, event, mut data| {
                let DispatchData{streams, state:State{..}} = data.get().unwrap();
                match event {
                    pointer::Event::Leave{..} => quit(streams),
                    pointer::Event::Motion{/*surface_x, surface_y,*/..} => {},
                    pointer::Event::Button{/*button, state,*/..} => {},
                    _ => {},
                }
            });
        }
    };
    for seat in env.get_all_seats() { with_seat_data(&seat, |seat_data| handler(&seat, seat_data)); }
    let seat_listener = env.listen_for_seats(move |seat, seat_data, _| handler(&seat, seat_data));

    mod nix {
        pub type RawPollFd = std::os::unix::io::RawFd;
        pub trait AsRawPollFd { fn as_raw_poll_fd(&self) -> RawPollFd; }
        impl AsRawPollFd for std::os::unix::io::RawFd { fn as_raw_poll_fd(&self) -> RawPollFd { *self } }
    }
    struct Async<T>(T);
    struct AsRawFd<T>(T);
    impl<T:nix::AsRawPollFd> std::os::unix::io::AsRawFd for AsRawFd<T> { fn as_raw_fd(&self) -> std::os::unix::io::RawFd { self.0.as_raw_poll_fd() /*->smol::Reactor*/ } }
    impl<T:nix::AsRawPollFd> Async<T> { fn new(io: T) -> Result<smol::Async<AsRawFd<T>>, std::io::Error> { smol::Async::new(AsRawFd(io)) } }
    impl nix::AsRawPollFd for client::EventQueue { fn as_raw_poll_fd(&self) -> nix::RawPollFd { self.display().get_connection_fd() } }

    let mut state = State {
        pool: env.create_simple_pool(|_|{})?,
        widget,
        unscaled_size: size2{x:0,y:0}
    };

    async move /*queue*/ {
        let poll_queue = Async::new(queue.display().create_event_queue())?;  // Registers in the reactor (borrows after moving queue in the async)
        let mut streams = SelectAll::new().peekable();

        streams.get_mut().push(
            unfold(poll_queue, async move |q| { // Apply message callbacks (&mut state)
                Some((Item::Apply(q.read_with(
                    |q| q.0.prepare_read().ok_or(std::io::Error::new(std::io::ErrorKind::Interrupted, "Dispatch all events before polling"))?.read_events()
                    ).await), q))
            }).boxed_local()
        );

        'run: loop {
            while let Some(item) = /*~poll_next*/ std::pin::Pin::new(&mut streams).peek().now_or_never() {
                let item = item.ok_or(std::io::Error::new(std::io::ErrorKind::UnexpectedEof,""))?;
                match item {
                    Item::Apply(_) => queue.dispatch_pending(/*Any: 'static*/unsafe{&mut erase_lifetime(DispatchData{streams: &mut streams, state: &mut state})}, |_,_,_| ())?,
                    Item::Quit => break 'run,
                };
                let _next = streams.next();
                assert!(_next.now_or_never().is_some());
            }
            queue.display().flush().unwrap();
            std::pin::Pin::new(&mut streams).peek().await;
        }
        drop(seat_listener);
        Ok(())
    }
}

#[throws] pub fn run(widget: &mut dyn Widget) { smol::run(window(widget)?)? }
