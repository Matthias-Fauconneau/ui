mod nix {
	pub type RawPollFd = std::os::unix::io::RawFd;
	pub trait AsRawPollFd { fn as_raw_poll_fd(&self) -> RawPollFd; }
	impl AsRawPollFd for std::os::unix::io::RawFd { fn as_raw_poll_fd(&self) -> RawPollFd { *self } }
}
struct Async<T>(T);
struct AsRawFd<T>(T);
impl<T:nix::AsRawPollFd> std::os::unix::io::AsRawFd for AsRawFd<T> { fn as_raw_fd(&self) -> std::os::unix::io::RawFd { self.0.as_raw_poll_fd() /*->smol::Reactor*/ } }
impl<T:nix::AsRawPollFd> Async<T> { fn new(io: T) -> Result<smol::Async<AsRawFd<T>>, std::io::Error> { smol::Async::new(AsRawFd(io)) } }
impl nix::AsRawPollFd for client_toolkit::reexports::client::EventQueue { fn as_raw_poll_fd(&self) -> nix::RawPollFd { self.display().get_connection_fd() } }

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

default_environment!(Compositor,
	fields = [ layer_shell: SimpleGlobal<LayerShell> ],
	singles = [ LayerShell => layer_shell ]
);

enum Item {
	Apply(std::io::Result<()>),
	KeyRepeat(Key),
	Quit,
}

use {core::{error::{throws, Error, Result}, num::{Zero, div_ceil}}, ::xy::{xy, size}, image::bgra8, crate::widget::{Widget, Target, Key, Event, ModifiersState}};

struct State<'w> {
	pool: shm::MemPool,
	modifiers_state: ModifiersState,
	surface: client::Attached<Surface>,
	widget: &'w mut dyn Widget,
	size: size,
	unscaled_size: size
}

use {std::iter::once, futures::{FutureExt, stream::{unfold, iter, StreamExt, SelectAll, LocalBoxStream}}};

type Streams<'q> = SelectAll<LocalBoxStream<'q, Item>>;
struct DispatchData<'d, 'q, 'w> {
	streams: &'d mut Streams<'q>,
	state: &'d mut State<'w>,
}

fn quit(streams: &mut Streams<'_>) { streams.push(iter(once(Item::Quit)).boxed_local()) }

#[throws] fn draw(pool: &mut shm::MemPool, surface: &Surface, widget: &mut dyn Widget, size: size) {
	assert!(size.x < 124839 || size.y < 1443);
	let stride = size.x*4;
	pool.resize((size.y*stride) as usize)?;
	let mut target = Target::from_bytes(pool.mmap(), size);
	image::fill(&mut target, bgra8{b:0,g:0,r:0,a:0xFF});
	widget.paint(&mut target)?;
	let buffer = pool.buffer(0, size.x as i32, size.y as i32, stride as i32, shm::Format::Argb8888);
	surface.attach(Some(&buffer), 0, 0);
	surface.damage_buffer(0, 0, size.x as i32, size.y as i32);
	surface.commit()
}

// DispatchData wraps using :Any which currently erases lifetimes
unsafe fn erase_lifetime<'d,'q,'w>(data: DispatchData<'d,'q,'w>) -> DispatchData<'static,'static,'static> {
	std::mem::transmute::<DispatchData::<'d,'q,'w>, DispatchData::<'static,'static,'static>>(data)
}

fn key(State{modifiers_state, pool, surface, widget, size, ..}: &mut State, key: Key) -> bool {
	use Key::*;
	match key {
		Escape => true,
		key => { if widget.event(&Event{modifiers_state: *modifiers_state, key}) { draw(pool, surface, *widget, *size).unwrap() }; false },
	}
}

#[throws] pub fn window<'w>(widget: &'w mut (dyn Widget + 'w)) -> impl std::future::Future<Output=Result<()>>+'w {
    let (env, _, mut queue) = init_default_environment!(Compositor, fields = [layer_shell: SimpleGlobal::new()])?;

    let surface = env.create_surface_with_scale_callback(|scale, surface, mut data| {
        let DispatchData{state:State{pool, widget, ref mut size, unscaled_size, ..}, ..} = data.get().unwrap(); //unsafe{restore_erased_lifetime(data.get().unwrap())};
        *size = (scale as u32) * *unscaled_size;
        surface.set_buffer_scale(scale);
        draw(pool, &surface, *widget, *size).unwrap();
    });

    let layer_shell = env.require_global::<LayerShell>();
    let layer_surface = layer_shell.get_layer_surface(&surface, None, layer_shell::Layer::Overlay, "framework".to_string());
    layer_surface.set_keyboard_interactivity(1);
    surface.commit();

    let handler = move |seat: &client::Attached<Seat>, seat_data: &SeatData| {
        if seat_data.has_keyboard {
			use std::{rc::Rc, cell::Cell};
			let mut repeat : Option<Rc<Cell<_>>> = None;
            seat.get_keyboard().quick_assign(move |_, event, mut data| {
				let DispatchData{streams, state} = data.get().unwrap();
                use keyboard::{Event::*, KeyState};
                match event {
                    Keymap {..} => {},
                    Enter { /*keysyms,*/ .. } => {},
                    Leave { .. } => {}
                    Key {state: key_state, key, .. } => {
						use std::convert::TryFrom;
						let key = crate::widget::Key::try_from(key as u8).unwrap_or_else(|_| panic!("{:x}", key));
						match key_state {
							KeyState::Released => if repeat.as_ref().filter(|r| r.get()==key ).is_some() { repeat = None },
							KeyState::Pressed => {
								if self::key(state, key) { quit(streams); }
								if let Some(repeat) = repeat.as_mut() { // Update existing repeat cell
									repeat.set(key);
									// Note: This keeps the same timer on key repeat change. No delay! Nice!
								} else { // New repeat timer (registers in the reactor on first poll)
									repeat = {
										let repeat = Rc::new(Cell::new(key));
										use futures::stream;
										streams.push(
											stream::unfold(std::time::Instant::now()+std::time::Duration::from_millis(150), {
												let repeat = Rc::downgrade(&repeat);
												move |last| {
													let next = last+std::time::Duration::from_millis(33);
													use async_io::Timer;
													Timer::at(next).map({
														let repeat = repeat.clone();
														move |_| { repeat.upgrade().map(|x| (Item::KeyRepeat(x.get()), next) ) } // Option<Key> (None stops the stream, autodrops from streams)
													})
												}
											}).boxed_local()
										);
										Some(repeat)
									}
								}
							},
							_ => unreachable!(),
						}
					},
                    Modifiers {mods_depressed, mods_latched, mods_locked, group: locked_group, ..} => {
                        assert_eq!([mods_latched, mods_locked, locked_group], [0,0,0]);
                        #[macro_export] macro_rules! assert_matches { ($expr:expr, $($pattern:tt)+) => { match $expr {
							$($pattern)+ => (),
							ref e => panic!("assertion failed: `{:?}` does not match `{}`", e, stringify!($($pattern)+)),
						}}}
						const CTRL : u32 = 0b100;
                        assert_matches!(mods_depressed, 0|CTRL);
						state.modifiers_state = ModifiersState {
							ctrl: mods_depressed&CTRL != 0,
							..Default::default()
						}
                    },
                    RepeatInfo {..} => {},
                    _ => unreachable!()
                }
            });
        }
        if seat_data.has_pointer {
            seat.get_pointer().quick_assign(|_, event, mut data| {
                let DispatchData{streams, ..} = data.get().unwrap();
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

    let mut state = State {
        pool: env.create_simple_pool(|_|{})?,
        modifiers_state: Default::default(),
        surface,
        widget,
        size: Zero::zero(),
        unscaled_size: Zero::zero()
    };

	layer_surface.quick_assign(move |layer_surface, event, mut data| {
        let DispatchData{streams, state:State{pool, surface, widget, ref mut size, ref mut unscaled_size, ..}} = data.get().unwrap();
        use layer_surface::Event::*;
        match event {
            Closed => quit(streams),
            Configure{serial, width, height} => {
                if !(width > 0 && height > 0) {
                    let (scale, size) = with_output_info(env.get_all_outputs().first().unwrap(),
																			|info| (info.scale_factor as u32, ::xy::int2::from(info.modes.first().unwrap().dimensions).into()) ).unwrap();
                    let size = core::vector::component_wise_min(size, widget.size(size));
                    assert!(size.x < 124839 || size.y < 1443, size);
                    layer_surface.set_size(div_ceil(size.x, scale), div_ceil(size.y, scale));
                    layer_surface.ack_configure(serial);
                    surface.commit();
                } else {
					layer_surface.ack_configure(serial);
					*unscaled_size = xy{x: width, y: height};
					let scale = if get_surface_outputs(&surface).is_empty() { // get_surface_outputs defaults to 1 instead of first output factor
						env.get_all_outputs().first().map(|output| with_output_info(output, |info| info.scale_factor)).flatten().unwrap_or(1)
					} else {
						get_surface_scale_factor(&surface)
					};
					*size = (scale as u32) * *unscaled_size;
					surface.set_buffer_scale(scale);
					draw(pool, &surface, *widget, *size).unwrap();
				}
            }
            _ => unimplemented!(),
        }
    });

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
                    Item::Apply(_) => { queue.dispatch_pending(/*Any: 'static*/unsafe{&mut erase_lifetime(DispatchData{streams: streams.get_mut(), state: &mut state})}, |_,_,_| ())?; },
                    &Item::KeyRepeat(key) => if self::key(&mut state, key) { break 'run; },
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
