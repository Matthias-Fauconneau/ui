use futures::stream::{StreamExt, unfold, LocalBoxStream, SelectAll, select_all};
use client_toolkit::{
	default_environment, environment::{Environment, SimpleGlobal}, new_default_environment,
	seat::{SeatListener, with_seat_data}, output::with_output_info, get_surface_outputs, get_surface_scale_factor, shm::{MemPool, Format},
	reexports::{
		client::{Display, EventQueue, Main, Attached, protocol::wl_surface::WlSurface as Surface},
		protocols::xdg_shell::client::{xdg_wm_base::{XdgWmBase as WmBase}, xdg_surface::{self, XdgSurface}, xdg_toplevel as toplevel},
	},
};
use {fehler::throws, error::Error, num::zero, ::xy::{xy, size}, crate::widget::{Widget, Target, EventContext, ModifiersState, Event}};

default_environment!(Compositor,
	fields = [ wm_base: SimpleGlobal<WmBase> ],
	singles = [ WmBase => wm_base ]
);

pub struct App<'t, W> {
	display: Option<Display>,
	pub streams: SelectAll<LocalBoxStream<'t, Box<dyn Fn/*Mut fixme*/(&mut Self)+'t>>>,
	pool: MemPool,
	_seat_listener: SeatListener,
	pub(crate) modifiers_state: ModifiersState,
	pub(crate) surface: Attached<Surface>,
	pub widget: W,
	pub(crate) size: size,
	unscaled_size: size,
	pub need_update: bool,
	//pub iterator: Box<dyn Iterator<Item=Box<dyn Fn(&mut Self)+'t>>>,
	pub idle: Box<dyn FnMut(&mut W)->bool>,
}

#[throws] fn draw(pool: &mut MemPool, surface: &Surface, widget: &mut dyn Widget, size: size) {
	assert!(size.x > 0 && size.y > 0 && size.x < 124839, "{:?}", size);
	let stride = size.x*4;
	pool.resize((size.y*stride) as usize)?;
	let mut target = Target::from_bytes(pool.mmap(), size);
	//image::fill(&mut target, bgra8{b:0,g:0,r:0,a:0xFF});
	widget.paint(&mut target)?;
	let buffer = pool.buffer(0, size.x as i32, size.y as i32, stride as i32, Format::Argb8888);
	surface.attach(Some(&buffer), 0, 0);
	surface.damage_buffer(0, 0, size.x as i32, size.y as i32);
	surface.commit()
}

fn surface<'t, W:Widget>(env: Environment<Compositor>) -> (Attached<Surface>, Main<XdgSurface>) {
	let surface = env.create_surface_with_scale_callback(|scale, surface, mut app| {
		let App{pool, widget, ref mut size, unscaled_size, ..} = unsafe{std::mem::transmute::<&mut App<&mut dyn Widget>,&mut App<'t,W>>(app.get::<App<&mut dyn Widget>>().unwrap())};
		*size = (scale as u32) * *unscaled_size;
		surface.set_buffer_scale(scale);
		draw(pool, &surface, widget, *size).unwrap()
	});

	let wm = env.require_global::<WmBase>(); // GlobalHandler<xdg_wm_base::XdgWmBase>::get assigns ping-pong
	let xdg_surface = wm.get_xdg_surface(&surface);
	let toplevel = xdg_surface.get_toplevel();
	surface.commit();

	toplevel.quick_assign(|_toplevel, event, mut app| {
		let App{display, unscaled_size, ..} = unsafe{std::mem::transmute::<&mut App<&mut dyn Widget>,&mut App<'t,W>>(app.get::<App<&mut dyn Widget>>().unwrap())};
		use toplevel::Event::*;
		match event {
			Close => *display = None,
			//Configure{..} => {}
			Configure{width, height, ..} => { *unscaled_size=xy{x:width as u32, y:height as u32}; }
			_ => unimplemented!(),
		}
	});
	xdg_surface.quick_assign(move /*env*/ |xdg_surface, event, mut app| {
		let App{pool, surface, widget, ref mut size, ref mut unscaled_size, ..} = unsafe{std::mem::transmute::<&mut App<&mut dyn Widget>,&mut App<'t,W>>(app.get::<App<&mut dyn Widget>>().unwrap())};
		use xdg_surface::Event::*;
		match event {
			Configure{serial} => {
				if !(unscaled_size.x > 0 && unscaled_size.y > 0) {
					let (scale, size) = with_output_info(env.get_all_outputs().first().unwrap(), |info| (info.scale_factor as u32, ::xy::int2::from(info.modes.first().unwrap().dimensions).into()) ).unwrap();
					let size = vector::component_wise_min(size, widget.size(size));
					assert!(size.x > 0 && size.y > 0 && size.x < 124839, "{:?}", size);
					*unscaled_size = ::xy::div_ceil(size, scale);
					//xdg_surface.set_window_geometry(.., unscaled_size.x, unscaled_size.y); // If never set, the value is the full bounds of the surface
				}
				xdg_surface.ack_configure(serial);
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
	(surface, xdg_surface)
}

use std::{rc::Rc, cell::RefCell};
#[throws] fn queue<'t, W:Widget>(queue: EventQueue) -> LocalBoxStream<'t, Box<dyn Fn(&mut App<'t, W>)+'t>> {
	let queue = Rc::new(RefCell::new(crate::as_raw_poll_fd::Async::new(queue)?)); // Rc simpler than an App.streams:&queue self-ref
	unfold(queue, async move |q| {
		q.borrow().read_with(|q| q.prepare_read().ok_or(std::io::Error::new(std::io::ErrorKind::Interrupted, "Dispatch all events before polling"))?.read_events()).await.unwrap();
		Some(({
			let q = q.clone();
			(box move |mut app| {
				//trace::timeout(100, || {
					q.borrow_mut().get_mut().dispatch_pending(/*Any: 'static*/unsafe{std::mem::transmute::<&mut App<'t, W>, &mut App<'static,&mut dyn Widget>>(&mut app)}, |_,_,_| ()).unwrap();
					app.draw();
				//})
			}) as Box<dyn Fn(&mut _/*App<'t, W>*/)>
		}, q))
	}).boxed_local()
}

impl<'t, W:Widget> App<'t, W> {
#[throws] pub fn new(widget: W) -> Self {
	let (env, display, queue) = new_default_environment!(Compositor, fields = [wm_base: SimpleGlobal::new()])?;
	let theme_manager = client_toolkit::seat::pointer::ThemeManager::init(client_toolkit::seat::pointer::ThemeSpec::System, env.require_global(), env.require_global());
	for s in env.get_all_seats() { with_seat_data(&s, |seat_data| crate::input::seat::<W>(&theme_manager, &s, seat_data)); }
	let _seat_listener = env.listen_for_seats(move /*theme_manager*/ |s, seat_data, _| crate::input::seat::<W>(&theme_manager, &s, seat_data));
	let pool = env.create_simple_pool(|_|{})?;
	let (surface, _) = surface::<W>(env);
	display.flush().unwrap();
	Self {
			display: Some(display),
			streams: select_all({let mut v=Vec::new(); v.push(self::queue(queue)?); v}),
			_seat_listener,
			modifiers_state: Default::default(),
			pool,
			surface,
			widget,
			size: zero(),
			unscaled_size: zero(),
			need_update: false,
			idle: box|_| false,
	}
}
pub async fn display(&mut self) {
	while let Some(event) = std::pin::Pin::new(&mut self.streams).next().await {
		event(self);
		if self.display.is_none() { break; }
		if (self.idle)(&mut self.widget) { self.need_update = true; } // Simpler than streams
	}
}
pub fn draw(&mut self) {
	let Self{display, pool, widget, size, surface, need_update, /*unscaled_size,*/ ..} = self;
	/*let max_size = with_output_info(get_surface_outputs(&surface).first().unwrap(), |info| ::xy::int2::from(info.modes.first().unwrap().dimensions).into()).unwrap();
	let widget_size = ::min(size, widget.size(max_size));
	if *size != widget_size {
		let scale = get_surface_scale_factor(&surface) as u32;
		*unscaled_size = ::xy::div_ceil(widget_size, scale);
		//self.xdg_surface.set_size(unscaled_size.x, unscaled_size.y);
		*size = (scale as u32) * *unscaled_size;
	}*/
	if *need_update { draw(pool, &surface, widget, *size).unwrap(); *need_update = false; }
	display.as_ref().map(|d| d.flush().unwrap());
}
pub fn quit(&mut self) { self.display = None }
#[throws] pub fn key(&mut self, key: char) -> bool {
	let Self{size, modifiers_state, widget, ..} = self;
	if widget.event(*size, &EventContext{modifiers_state: *modifiers_state, pointer: None}, &Event::Key{key})? { self.need_update = true; true }
	else if key == '⎋' { self.quit(); false }
	else { false }
}
#[throws] pub fn run(mut self) { async_io::block_on(self.display()) }
}
#[throws] pub fn run(widget: impl Widget) { App::new(widget)?.run()? }
