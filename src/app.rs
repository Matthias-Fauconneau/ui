use {fehler::throws, num::zero, ::xy::{xy, size}, crate::{Error, Result, widget::{Widget, EventContext, ModifiersState, Event}}};
use futures::stream::{StreamExt, unfold, LocalBoxStream, SelectAll, select_all};
use client_toolkit::{
	default_environment, environment::{Environment, SimpleGlobal}, new_default_environment,
	seat::{SeatListener, with_seat_data}, output::with_output_info, get_surface_outputs, get_surface_scale_factor,
	reexports::{
		client::{Display, EventQueue, Main, Attached, DispatchData, protocol::wl_surface::WlSurface as Surface},
		protocols::xdg_shell::client::{xdg_wm_base::XdgWmBase as WmBase, xdg_surface::{self as surface, XdgSurface}, xdg_toplevel as toplevel},
	},
};

default_environment!(Compositor,
	fields = [ wm_base: SimpleGlobal<WmBase> ],
	singles = [ WmBase => wm_base ]
);

pub struct App<'t, W> {
	display: Option<Display>,
	pub streams: SelectAll<LocalBoxStream<'t, Box<dyn FnOnce(&mut Self)->Result<()>+'t>>>,
	_seat_listener: SeatListener,
	pub(crate) modifiers_state: ModifiersState,
	pub(crate) surface: Attached<Surface>,
	instance: piet_gpu_hal::Instance,
	gpu_surface: piet_gpu_hal::Surface,
	pub widget: W,
	pub(crate) size: size,
	unscaled_size: size,
	pub need_update: bool,
}

pub(crate) fn deref_mut<'t:'d, 'd, W>(mut app: DispatchData<'d>) -> &'d mut App<'t,W> {  
	unsafe{std::mem::transmute::<&mut App<&mut dyn Widget>,&mut App<'t,W>>(app.get::<App<&mut dyn Widget>>().unwrap())} 
}

#[throws] fn draw(instance: &piet_gpu_hal::Instance, surface: &piet_gpu_hal::Surface, widget: &mut dyn Widget, size: size) {
	let device = unsafe{instance.device(Some(&surface))}?;
	let mut swapchain = unsafe{instance.swapchain(size.x as _, size.y as _, &device, &surface)}?;
	let session = piet_gpu_hal::Session::new(device);
	let present_semaphore = unsafe{session.create_semaphore()}?;
	let mut renderer = unsafe{piet_gpu::Renderer::new(&session, size.x as _, size.y as _, 1)}?;
	let mut context = piet_gpu::PietGpuRenderContext::new();
	context.fill(piet::kurbo::Rect::new(0., 0., size.x as _, size.y as _), piet::Color::WHITE);
	widget.paint(&mut context, size)?;
	if context.path_count() > 0 {
		renderer.upload_render_ctx(&mut context, 0)?;
	}
	let (image_idx, acquisition_semaphore) = unsafe{swapchain.next()}?;
	let swap_image = unsafe{swapchain.image(image_idx)};
	let ref query_pool = session.create_query_pool(8)?;
	let mut cmd_buf = session.cmd_buf()?;
	unsafe{
		cmd_buf.begin();
		renderer.record(&mut cmd_buf, &query_pool, 0);
		use piet_gpu_hal::ImageLayout;
		cmd_buf.image_barrier(&swap_image, ImageLayout::Undefined, ImageLayout::BlitDst);
		cmd_buf.blit_image(&renderer.image_dev, &swap_image);
		cmd_buf.image_barrier(&swap_image, ImageLayout::BlitDst, ImageLayout::Present);
		cmd_buf.finish();
		let submitted = session.run_cmd_buf(cmd_buf, &[&acquisition_semaphore], &[&present_semaphore])?;
		swapchain.present(0, &[&present_semaphore])?;
		submitted.wait()?;
	}
}

fn surface<'t, W:Widget>(env: Environment<Compositor>) -> (Attached<Surface>, Main<XdgSurface>) {
	let surface = env.create_surface_with_scale_callback(|scale, surface, app| {
		let app = deref_mut::<'t, '_, W>(app);
		app.size = (scale as u32) * app.unscaled_size;
		surface.set_buffer_scale(scale);
		let App{instance, gpu_surface, widget, size, ..} = app;
		draw(instance, gpu_surface, widget, *size).unwrap()
	});

	let wm = env.require_global::<WmBase>();
	// GlobalHandler<xdg_wm_base::XdgWmBase>::get assigns ping-pong
    let xdg_surface = wm.get_xdg_surface(&surface);
	let toplevel = xdg_surface.get_toplevel();
	surface.commit();

	toplevel.quick_assign(|_toplevel, event, app| {
		let App{display, unscaled_size, ..} = deref_mut::<W>(app);
		use toplevel::Event::*;
		match event {
			Close => *display = None,
			//Configure{..} => {}
			Configure{width, height, ..} => { *unscaled_size=xy{x:width as u32, y:height as u32}; }
			_ => unimplemented!(),
		}
	});
	xdg_surface.quick_assign(move /*env*/ |xdg_surface, event, app| {
		let App{instance, surface, gpu_surface, widget, ref mut size, ref mut unscaled_size, ..} = deref_mut::<W>(app);
		use surface::Event::*;
		match event {
			Configure{serial} => {
				if !(unscaled_size.x > 0 && unscaled_size.y > 0) {
					let (scale, size) = with_output_info(env.get_all_outputs().first().unwrap(), |info| (info.scale_factor as u32, ::xy::int2::from(info.modes.first().unwrap().dimensions).into()) ).unwrap();
					let size = vector::component_wise_min(size, widget.size(size));
					assert!(size.x > 0 && size.y > 0 && size.x < 124839, "{:?}", size);
					*unscaled_size = ::xy::div_ceil(size, scale);
				}
				xdg_surface.ack_configure(serial);
				let scale = if get_surface_outputs(&surface).is_empty() { // get_surface_outputs defaults to 1 instead of first output factor
					env.get_all_outputs().first().map(|output| with_output_info(output, |info| info.scale_factor)).flatten().unwrap_or(1)
				} else {
					get_surface_scale_factor(&surface)
				};
				*size = (scale as u32) * *unscaled_size;
				surface.set_buffer_scale(scale);
				draw(instance, &gpu_surface, widget, *size).unwrap()
			}
			_ => unimplemented!(),
		}
	});
	(surface, xdg_surface)
}

use std::{rc::Rc, cell::RefCell};
#[throws] fn queue<'t, W:Widget>(queue: EventQueue) -> LocalBoxStream<'t, Box<dyn FnOnce(&mut App<'t, W>)->Result<()>+'t>> {
	let queue = Rc::new(RefCell::new(crate::as_raw_poll_fd::Async::new(queue)?)); // Rc simpler than a App.streams:&queue self-ref
	unfold(queue, async move |q| {
		let _ = q.borrow().read_with(|q| q.prepare_read().ok_or(std::io::Error::new(std::io::ErrorKind::Interrupted, "Dispatch all events before polling"))?.read_events()).await;//.unwrap();
		Some(({
			let q = q.clone();
			(box move |mut app| {
				//trace::timeout(100, || {
					q.borrow_mut().get_mut().dispatch_pending(/*Any: 'static*/unsafe{std::mem::transmute::<&mut App<'t, W>, &mut App<'static,&mut dyn Widget>>(&mut app)}, |_,_,_| ()).unwrap();
					if app.need_update { app.draw()? }
					Ok(())
				//})
			}) as Box<dyn FnOnce(&mut _)->Result<()>>
		}, q))
	}).boxed_local()
}

impl<'t, W:Widget> App<'t, W> {
#[throws] pub fn new(widget: W) -> Self {
	let (env, display, queue) = new_default_environment!(Compositor, fields = [wm_base: SimpleGlobal::new()])?;
	let theme_manager = client_toolkit::seat::pointer::ThemeManager::init(client_toolkit::seat::pointer::ThemeSpec::System, env.require_global(), env.require_global());
	for s in env.get_all_seats() { with_seat_data(&s, |seat_data| crate::input::seat::<W>(&theme_manager, &s, seat_data)); }
	let _seat_listener = env.listen_for_seats(move /*theme_manager*/ |s, seat_data, _| crate::input::seat::<W>(&theme_manager, &s, seat_data));
	let (surface, _) = surface::<W>(env);
	display.flush().unwrap();
	struct RawWindowHandle(raw_window_handle::RawWindowHandle);
	unsafe impl raw_window_handle::HasRawWindowHandle for RawWindowHandle { fn raw_window_handle(&self) -> raw_window_handle::RawWindowHandle { self.0 } }
	let (instance, gpu_surface) = piet_gpu_hal::Instance::new(Some(&RawWindowHandle(raw_window_handle::RawWindowHandle::Wayland({let mut s=raw_window_handle::unix::WaylandHandle::empty(); s.display=display.get_display_ptr() as *mut _; s.surface=surface.as_ref().c_ptr() as *mut _; s}))), Default::default())?;
	Self {
			display: Some(display),
			streams: select_all({let mut v=Vec::new(); v.push(self::queue(queue)?); v}),
			_seat_listener,
			modifiers_state: ModifiersState::default(),
			surface,
			instance,
			gpu_surface: gpu_surface.unwrap(),
			widget,
			size: zero(),
			unscaled_size: zero(),
			need_update: true,
	}
}
pub async fn display(&mut self, mut idle: impl FnMut(&mut W)->Result<bool>) -> Result<()> {
	while let Some(event) = std::pin::Pin::new(&mut self.streams).next().await {
		event(self)?;
		if self.display.is_none() { break; }
		if idle(&mut self.widget)? { self.need_update = true; }
	}
	Ok(())
}
#[throws] pub fn draw(&mut self) {
	let Self{display, instance, gpu_surface, widget, size, need_update, ..} = self;
	if *size == (xy{x: 0, y: 0}) { return; }
	draw(instance, &gpu_surface, widget, *size)?;
	*need_update = false;
	display.as_ref().map(|d| d.flush().unwrap());
}
pub fn quit(&mut self) { self.display = None }
#[throws] pub fn key(&mut self, key: char) -> bool {
	let Self{size, modifiers_state, widget, ..} = self;
	if widget.event(*size, &EventContext{modifiers_state: *modifiers_state, pointer: None}, &Event::Key{key})? { self.need_update = true; true }
	else if key == '⎋' { self.quit(); false }
	else { false }
}
pub fn run(mut self, idle: impl FnMut(&mut W)->Result<bool>) -> Result<()> { async_io::block_on(self.display(idle)) }
}
#[throws] pub fn run(widget: impl Widget) { App::new(widget)?.run(|_| Ok(false))? }
