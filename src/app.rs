use futures::stream::{StreamExt, unfold, LocalBoxStream, SelectAll, select_all};
use client_toolkit::{
	default_environment, environment::{Environment, SimpleGlobal}, init_default_environment,
	seat::{SeatListener, with_seat_data}, output::with_output_info, get_surface_outputs, get_surface_scale_factor, shm::{MemPool, Format},
	reexports::{
		client::{Display, EventQueue, Main, Attached, protocol::wl_surface::WlSurface as Surface},
		protocols::wlr::unstable::layer_shell::v1::client::{
			zwlr_layer_shell_v1::{self as layer_shell, ZwlrLayerShellV1 as LayerShell},
			zwlr_layer_surface_v1::{self  as layer_surface, ZwlrLayerSurfaceV1 as LayerSurface},
		},
	},
};
use {fehler::throws, error::Error, num::Zero, ::xy::{xy, size}, image::bgra8, crate::widget::{Widget, Target, EventContext, ModifiersState, Event}};

default_environment!(Compositor,
	fields = [ layer_shell: SimpleGlobal<LayerShell> ],
	singles = [ LayerShell => layer_shell ]
);

pub struct App<'t, W> {
	display: Option<Display>,
	pub streams: SelectAll<LocalBoxStream<'t, Box<dyn Fn(&mut Self)+'t>>>,
	pool: MemPool,
	_seat_listener: SeatListener,
	pub(crate) modifiers_state: ModifiersState,
	layer_surface: Main<LayerSurface>,
	pub(crate) surface: Attached<Surface>,
	pub(crate) widget: W,
	pub(crate) size: size,
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

fn surface<'t, W:Widget>(env: Environment<Compositor>) -> (Attached<Surface>, Main<LayerSurface>) {
	let surface = env.create_surface_with_scale_callback(|scale, surface, mut app| {
		let App{pool, widget, ref mut size, unscaled_size, ..} = unsafe{std::mem::transmute::<&mut App<&mut dyn Widget>,&mut App<'t,W>>(app.get::<App<&mut dyn Widget>>().unwrap())};
		*size = (scale as u32) * *unscaled_size;
		surface.set_buffer_scale(scale);
		draw(pool, &surface, widget, *size).unwrap()
	});

	let layer_shell = env.require_global::<LayerShell>();
	let layer_surface = layer_shell.get_layer_surface(&surface, None, layer_shell::Layer::Overlay, "ui".to_string());
	surface.commit();

	layer_surface.quick_assign(move /*env*/ |layer_surface, event, mut app| {
		let App{display, pool, surface, widget, ref mut size, ref mut unscaled_size, ..} = unsafe{std::mem::transmute::<&mut App<&mut dyn Widget>,&mut App<'t,W>>(app.get::<App<&mut dyn Widget>>().unwrap())};
		use layer_surface::Event::*;
		match event {
			Closed => *display = None,
			Configure{serial, width, height} => {
				if !(width > 0 && height > 0) {
					let (scale, size) = with_output_info(env.get_all_outputs().first().unwrap(), |info| (info.scale_factor as u32, ::xy::int2::from(info.modes.first().unwrap().dimensions).into()) ).unwrap();
					let size = vector::component_wise_min(size, widget.size(size));
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
	(surface, layer_surface)
}

use std::{rc::Rc, cell::RefCell};
#[throws] fn queue<'t, W:Widget>(queue: EventQueue) -> LocalBoxStream<'t, Box<dyn Fn(&mut App<'t, W>)+'t>> {
	let queue = Rc::new(RefCell::new(crate::as_raw_poll_fd::Async::new(queue)?)); // Rc simpler than an App.streams:&queue self-ref
	unfold(queue, async move |q| {
		q.borrow().read_with(|q| q.prepare_read().ok_or(std::io::Error::new(std::io::ErrorKind::Interrupted, "Dispatch all events before polling"))?.read_events()).await.unwrap();
		Some(({
						let q = q.clone();
						(box move |mut app| {
								q.borrow_mut().get_mut().dispatch_pending(/*Any: 'static*/unsafe{std::mem::transmute::<&mut App<'t, W>, &mut App<'static,&mut dyn Widget>>(&mut app)}, |_,_,_| ()).unwrap();
								app.display.as_ref().map(|d| d.flush().unwrap());
						}) as Box<dyn Fn(&mut _/*App<'t, W>*/)>
				}, q))
	}).boxed_local()
}

impl<'t, W:Widget> App<'t, W> {
#[throws] pub fn new(widget: W) -> Self {
	let (env, display, queue) = init_default_environment!(Compositor, fields = [layer_shell: SimpleGlobal::new()])?;
	let theme_manager = client_toolkit::seat::pointer::ThemeManager::init(client_toolkit::seat::pointer::ThemeSpec::System, env.require_global(), env.require_global());
	for s in env.get_all_seats() { with_seat_data(&s, |seat_data| crate::input::seat::<W>(&theme_manager, &s, seat_data)); }
	let _seat_listener = env.listen_for_seats(move /*theme_manager*/ |s, seat_data, _| crate::input::seat::<W>(&theme_manager, &s, seat_data));
	let pool = env.create_simple_pool(|_|{})?;
	let (surface, layer_surface) = surface::<W>(env);
	display.flush().unwrap();
	Self {
			display: Some(display),
			streams: select_all({let mut v=Vec::new(); v.push(self::queue(queue)?); v}),
			_seat_listener,
			modifiers_state: Default::default(),
			pool,
			layer_surface,
			surface,
			widget,
			size: Zero::zero(),
			unscaled_size: Zero::zero()
	}
}
#[throws(std::io::Error)] pub async fn display(&mut self) { while let Some(event) = std::pin::Pin::new(&mut self.streams).next().await { event(self); if self.display.is_none() { break; } } }
pub fn draw(&mut self) {
	let Self{display, pool, widget, size, surface, unscaled_size, ..} = self;
	let max_size = with_output_info(get_surface_outputs(&surface).first().unwrap(), |info| ::xy::int2::from(info.modes.first().unwrap().dimensions).into()).unwrap();
	let widget_size = widget.size(max_size);
	if *size != widget_size {
		let scale = get_surface_scale_factor(&surface) as u32;
		*unscaled_size = ::xy::div_ceil(widget_size, scale);
		self.layer_surface.set_size(unscaled_size.x, unscaled_size.y);
		*size = (scale as u32) * *unscaled_size;
	}
	draw(pool, &surface, widget, *size).unwrap();
	display.as_ref().map(|d| d.flush().unwrap());
}
pub fn quit(&mut self) { self.display = None }
#[throws] pub fn key(&mut self, key: char) -> bool {
	let Self{size, modifiers_state, widget, ..} = self;
	if widget.event(*size, &EventContext{modifiers_state: *modifiers_state, pointer: None}, &Event::Key{key})? { self.draw(); true }
	else if key == '⎋' { self.quit(); false }
	else { false }
}
}
#[throws] pub fn run(widget: impl Widget) { async_io::block_on(App::new(widget)?.display())? }
