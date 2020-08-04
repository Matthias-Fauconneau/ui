mod nix {
	pub type RawPollFd = std::os::unix::io::RawFd;
	pub trait AsRawPollFd { fn as_raw_poll_fd(&self) -> RawPollFd; }
	impl AsRawPollFd for std::os::unix::io::RawFd { fn as_raw_poll_fd(&self) -> RawPollFd { *self } }
}
struct Async<T>(T);
struct AsRawFd<T>(T);
impl<T:nix::AsRawPollFd> std::os::unix::io::AsRawFd for AsRawFd<T> { fn as_raw_fd(&self) -> std::os::unix::io::RawFd { self.0.as_raw_poll_fd() /*->smol::Reactor*/ } }
impl<T> std::ops::Deref for AsRawFd<T> { type Target = T; fn deref(&self) -> &Self::Target { &self.0 } }
impl<T> std::ops::DerefMut for AsRawFd<T> { fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 } }
impl<T:nix::AsRawPollFd> Async<T> { fn new(io: T) -> Result<smol::Async<AsRawFd<T>>, std::io::Error> { smol::Async::new(AsRawFd(io)) } }
impl nix::AsRawPollFd for client_toolkit::reexports::client::EventQueue { fn as_raw_poll_fd(&self) -> nix::RawPollFd { self.display().get_connection_fd() } }

use std::convert::TryFrom;
use std::lazy::SyncLazy;
#[allow(non_upper_case_globals)] static usb_hid_usage_table: SyncLazy<Vec<char>> = SyncLazy::new(|| [
	&['\0','⎋','1','2','3','4','5','6','7','8','9','0','-','=','⌫','\t','q','w','e','r','t','y','u','i','o','p','{','}','\n','⌃','a','s','d','f','g','h','j','k','l',';','\'','`','⇧','\\','z','x','c','v','b','n','m',',','.','/','⇧','\0','⎇',' ','⇪'],
	&(1..=10).map(|i| char::try_from(0xF700u32+i).unwrap()).collect::<Vec<_>>()[..], &['\0'; 20], &['\u{F70B}','\u{F70C}'], &['\0'; 8],
	&['\n','⌃',' ','⎇','⇱','↑','⇞','←','→','⇲','↓','⇟','\u{8}','⌦']
].concat());

use client_toolkit::{
	default_environment, environment::{Environment, SimpleGlobal}, init_default_environment,
	seat::{SeatListener, SeatData, with_seat_data}, output::with_output_info, get_surface_outputs, get_surface_scale_factor, shm::{MemPool, Format},
	reexports::{
		client::{Display, EventQueue, Attached, protocol::{wl_surface::WlSurface as Surface, wl_seat::WlSeat as Seat, wl_keyboard as keyboard, wl_pointer as pointer}},
		protocols::wlr::unstable::layer_shell::v1::client::{zwlr_layer_shell_v1::{self as layer_shell, ZwlrLayerShellV1 as LayerShell}, zwlr_layer_surface_v1 as layer_surface},
	},
};

default_environment!(Compositor,
	fields = [ layer_shell: SimpleGlobal<LayerShell> ],
	singles = [ LayerShell => layer_shell ]
);

use futures::{FutureExt, stream::{unfold, StreamExt, LocalBoxStream, SelectAll, select_all}};
use {core::{error::{throws, Error, Result}, num::Zero}, ::xy::{xy, size}, image::bgra8, crate::widget::{Widget, Target, Event, ModifiersState}};
pub struct App<'t, W> {
	display: Option<Display>,
	pub streams: SelectAll<LocalBoxStream<'t, Box<dyn Fn(&mut Self)+'t>>>,
	pool: MemPool,
	_seat_listener: SeatListener,
	modifiers_state: ModifiersState,
	surface: Attached<Surface>,
	widget: W,
	size: size,
	unscaled_size: size
}

#[throws] fn draw(pool: &mut MemPool, surface: &Surface, widget: &mut dyn Widget, size: size) {
	assert!(size.x < 124839 || size.y < 1443);
	let stride = size.x*4;
	pool.resize((size.y*stride) as usize)?;
	let mut target = Target::from_bytes(pool.mmap(), size);
	image::fill(&mut target, bgra8{b:0,g:0,r:0,a:0xFF});
	widget.paint(&mut target)?;
	let buffer = pool.buffer(0, size.x as i32, size.y as i32, stride as i32, Format::Argb8888);
	surface.attach(Some(&buffer), 0, 0);
	surface.damage_buffer(0, 0, size.x as i32, size.y as i32);
	surface.commit()
}

use std::{rc::Rc, cell::Cell, cell::RefCell};
fn seat<'t, W:Widget>(seat: &Attached<Seat>, seat_data: &SeatData) {
    if seat_data.has_keyboard {
        let mut repeat : Option<Rc<Cell<_>>> = None;
        seat.get_keyboard().quick_assign(move |_, event, mut app| {
            //let app = app.get::<App<'t>>().unwrap();
            let app = unsafe{std::mem::transmute::<&mut App<&mut dyn Widget>,&mut App<'t,W>>(app.get::<App<&mut dyn Widget>>().unwrap())};
            use keyboard::{Event::*, KeyState};
            match event {
                Keymap {..} => {},
                Enter { /*keysyms,*/ .. } => {},
                Leave { .. } => {}
                Key {state, key, .. } => {
                    let key = *usb_hid_usage_table.get(key as usize).unwrap_or_else(|| panic!("{:x}", key));
                    match state {
                        KeyState::Released => if repeat.as_ref().filter(|r| r.get()==key ).is_some() { repeat = None },
                        KeyState::Pressed => {
                            app.key(key);
                            if let Some(repeat) = repeat.as_mut() { // Update existing repeat cell
                                repeat.set(key); // Note: This keeps the same timer on key repeat change. No delay! Nice!
                            } else { // New repeat timer (registers in the reactor on first poll)
                                repeat = {
                                    let repeat = Rc::new(Cell::new(key));
                                    app.streams.push(
                                        unfold(std::time::Instant::now()+std::time::Duration::from_millis(150), {
                                            let repeat = Rc::downgrade(&repeat);
                                            move |last| {
                                                let next = last+std::time::Duration::from_millis(33);
                                                use async_io::Timer;
                                                Timer::at(next).map({
                                                    let repeat = repeat.clone();
                                                    // stops and autodrops from streams when weak link fails to upgrade (repeat cell dropped)
                                                    move |_| { repeat.upgrade().map(|x| ({let key = x.get(); (box move |app| app.key(key)) as Box::<dyn Fn(&mut App<'t,_>)>}, next) ) }
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
                    const SHIFT : u32 = 0b001;
                    const CTRL : u32 = 0b100;
                    assert_eq!([mods_depressed&!(SHIFT|CTRL), mods_latched, mods_locked, locked_group], [0,0,0,0]);
                    app.modifiers_state = ModifiersState {
                        shift: mods_depressed&SHIFT != 0,
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
        seat.get_pointer().quick_assign(|_, event, mut app| {
						let App{display, ..} = unsafe{std::mem::transmute::<&mut App<&mut dyn Widget>,&mut App<'t,W>>(app.get::<App<&mut dyn Widget>>().unwrap())};
            match event {
                pointer::Event::Leave{..} => *display = None,
                pointer::Event::Motion{/*surface_x, surface_y,*/..} => {},
                pointer::Event::Button{/*button, state,*/..} => {},
                _ => {},
            }
        });
    }
}

fn surface<'t, W:Widget>(env: Environment<Compositor>) -> Attached<Surface> {
	let surface = env.create_surface_with_scale_callback(|scale, surface, mut app| {
		let App{pool, widget, ref mut size, unscaled_size, ..} = unsafe{std::mem::transmute::<&mut App<&mut dyn Widget>,&mut App<'t,W>>(app.get::<App<&mut dyn Widget>>().unwrap())};
		*size = (scale as u32) * *unscaled_size;
		surface.set_buffer_scale(scale);
		draw(pool, &surface, widget, *size).unwrap()
	});

	let layer_shell = env.require_global::<LayerShell>();
	let layer_surface = layer_shell.get_layer_surface(&surface, None, layer_shell::Layer::Overlay, "framework".to_string());
	layer_surface.set_keyboard_interactivity(1);
	surface.commit();

	layer_surface.quick_assign(move /*env*/ |layer_surface, event, mut app| {
		let App{display, pool, surface, widget, ref mut size, ref mut unscaled_size, ..} = unsafe{std::mem::transmute::<&mut App<&mut dyn Widget>,&mut App<'t,W>>(app.get::<App<&mut dyn Widget>>().unwrap())};
		use layer_surface::Event::*;
		match event {
			Closed => *display = None,
			Configure{serial, width, height} => {
				if !(width > 0 && height > 0) {
					let (scale, size) = with_output_info(env.get_all_outputs().first().unwrap(), |info| (info.scale_factor as u32, ::xy::int2::from(info.modes.first().unwrap().dimensions).into()) ).unwrap();
					let size = core::vector::component_wise_min(size, widget.size(size));
					assert!(size.x < 124839 || size.y < 1443, size);
					*unscaled_size = ::xy::div_ceil(size, scale);
					layer_surface.set_size(unscaled_size.x, unscaled_size.y);
					layer_surface.ack_configure(serial);
					surface.commit();
				} else {
					layer_surface.ack_configure(serial);
					*unscaled_size = xy{x: width, y: height};
				}
				let scale = if get_surface_outputs(&surface).is_empty() { // get_surface_outputs defaults to 1 instead of first output factor
					env.get_all_outputs().first().map(|output| with_output_info(output, |info| info.scale_factor)).flatten().unwrap_or(1)
				} else {
					get_surface_scale_factor(&surface)
				};
				*size = (scale as u32) * *unscaled_size;
				surface.set_buffer_scale(scale);
				draw(pool, &surface, widget, *size).unwrap();
			}
			_ => unimplemented!(),
		}
	});
	surface
}

impl<'t, W:Widget> App<'t, W> {
#[throws] pub fn new(widget: W) -> Self {
    let (env, display, queue) = init_default_environment!(Compositor, fields = [layer_shell: SimpleGlobal::new()])?;
    for s in env.get_all_seats() { with_seat_data(&s, |seat_data| seat::<W>(&s, seat_data)); }
	let _seat_listener = env.listen_for_seats(|s, seat_data, _| seat::<W>(&s, seat_data));
	let pool = env.create_simple_pool(|_|{})?;
    let surface = surface::<W>(env);
    display.flush().unwrap();
    Self {
        display: Some(display),
        streams: select_all({let mut v=Vec::new(); v.push(Self::queue(queue)?); v}),
        _seat_listener,
        modifiers_state: Default::default(),
        pool,
        surface,
        widget,
        size: Zero::zero(),
        unscaled_size: Zero::zero()
    }
}
#[throws] fn queue(queue: EventQueue) -> LocalBoxStream<'t, Box<dyn Fn(&mut Self)+'t>> {
	let queue = Rc::new(RefCell::new(Async::new(queue)?)); // Rc simpler than an App.streams:&queue self-ref
	unfold(queue, async move |q| {
		q.borrow().read_with(|q| q.0.prepare_read().ok_or(std::io::Error::new(std::io::ErrorKind::Interrupted, "Dispatch all events before polling"))?.read_events()).await.unwrap();
		Some(({
						let q = q.clone();
						(box move |mut app: &mut Self| {
								q.borrow_mut().get_mut().dispatch_pending(/*Any: 'static*/unsafe{std::mem::transmute::<&mut Self, &mut App<'static,&mut dyn Widget>>(&mut app)}, |_,_,_| ()).unwrap();
								app.display.as_ref().map(|d| d.flush().unwrap());
						}) as Box<dyn Fn(&mut Self)>
				}, q))
	}).boxed_local()
}
#[throws(std::io::Error)] pub async fn display(&mut self) { while let Some(event) = std::pin::Pin::new(&mut self.streams).next().await { event(self); if self.display.is_none() { break; } } }
pub fn draw(&mut self) { let Self{pool, surface, widget, size,..} = self; draw(pool, &surface, widget, *size).unwrap(); }
fn key(&mut self, key: char) {
    let Self{display, modifiers_state, widget, ..} = self;
    if widget.event(&Event{modifiers_state: *modifiers_state, key}) { self.draw(); }
	else if key == '⎋' { *display = None }
}
}
#[throws] pub fn run(widget: impl Widget) { smol::run(App::new(widget)?.display())? }
