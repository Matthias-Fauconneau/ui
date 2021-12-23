use {fehler::throws, crate::{Error, Result}};
use wayland_client::{Connection, Dispatch, ConnectionHandle, QueueHandle as Queue, DataInit, protocol::{
	wl_display::WlDisplay as Display,
	wl_registry::{self as registry, WlRegistry as Registry},
	wl_compositor::{self as compositor, WlCompositor as Compositor}, 
	wl_seat::WlSeat as Seat,
	wl_surface::{self as surface, WlSurface as Surface}
}};
use wayland_protocols::xdg_shell::client::{
	xdg_wm_base::{self as wm_base, XdgWmBase as WmBase},
	xdg_surface::{self, XdgSurface},
	xdg_toplevel::{self as toplevel, XdgToplevel as TopLevel}
};
//use client_toolkit::{default_environment, environment::{Environment, SimpleGlobal}, new_default_environment};
//use client_toolkit::{seat::{SeatListener, with_seat_data}, output::with_output_info, get_surface_outputs, get_surface_scale_factor};
use futures::stream::{StreamExt, LocalBoxStream, SelectAll, unfold, select_all};
use crate::widget::{Widget, /*EventContext, ModifiersState, Event*/};
use {::xy::{xy, size}, num::zero};

pub struct State<W> {
	pub widget: W,
	display: Display,
	pub streams: SelectAll<LocalBoxStream<'static, Box<dyn FnOnce(&mut Self)->Result<()>+'static>>>,
	wm_base: Option<WmBase>,
	crate surface: Option<Surface>,
	xdg_surface: Option<(XdgSurface, TopLevel)>,
	unscaled_size: size,
	crate size: size,
	pub need_update: bool,
	//crate modifiers_state: ModifiersState,
	//instance: piet_gpu_hal::Instance,
	//gpu_surface: Option<piet_gpu_hal::Surface>,
}

impl<W:Widget+'static> State<W> { #[throws] pub fn new(widget: W) -> Self { 
	let cx = Connection::connect_to_env()?;
    let mut event_queue = cx.new_event_queue::<State<W>>();
    let queue_handle = event_queue.handle();
    let display = cx.handle().display();
    display.get_registry(&mut cx.handle(), &queue_handle, ())?;
	let mut state = Self {
		widget,
		display,
		streams: select_all([{
			unfold(crate::as_raw_poll_fd::Async::new(cx)?, async move |cx| {
				cx.readable().await.unwrap();
				Some(({let mut queue = cx.get_ref().new_event_queue::<State<W>>(); (box move |state| Ok({println!("dispatch"); queue.dispatch_pending(state).unwrap();})) as Box<dyn FnOnce(&mut _)->Result<()>>}, cx))
			}).boxed_local()
		}]),
		wm_base: None,
		surface: None,
		xdg_surface: None,
		unscaled_size: zero(),
		size: zero(),
		need_update: true,
		//modifiers_state: ModifiersState::default(),
		//instance,
		//gpu_surface,
	};
	event_queue.blocking_dispatch(&mut state).unwrap();
	state
}}

impl<W: 'static> Dispatch<Registry> for State<W> {
    type UserData = ();
    fn event(&mut self, registry: &Registry, event: registry::Event, _: &Self::UserData, cx: &mut ConnectionHandle, queue: &Queue<Self>, _: &mut DataInit<'_>) {
		dbg!(&event);
		match event {
			registry::Event::Global{name, interface, ..} => match &interface[..] {
				"wl_compositor" => self.surface = Some(registry.bind::<Compositor, _>(cx, name, 1, queue, ()).unwrap().create_surface(cx, queue, ()).unwrap()),
				"wl_seat" => { registry.bind::<Seat, _>(cx, name, 1, queue, ()).unwrap(); }
				"xdg_wm_base" => self.wm_base = Some(registry.bind::<WmBase, _>(cx, name, 1, queue, ()).unwrap()),
				_ => {}
			},
			_ => {}
		};
		if let (Some(wm_base), Some(surface)) = (self.wm_base.as_ref(), self.surface.as_ref()) {
			let xdg_surface = wm_base.get_xdg_surface(cx, surface.clone(), queue, ()).unwrap();
			let toplevel = xdg_surface.get_toplevel(cx, queue, ()).unwrap();
			toplevel.set_title(cx, "Simple Rust Wayland Application".into());
			surface.commit(cx);
			self.xdg_surface = Some((xdg_surface, toplevel));
		}
	}
}

impl<W> Dispatch<Compositor> for State<W> {
    type UserData = ();
    fn event(&mut self, _: &Compositor, _: compositor::Event, _: &Self::UserData, _: &mut ConnectionHandle, _: &Queue<Self>, _: &mut DataInit<'_>) {}
}

impl<W> Dispatch<Surface> for State<W> {
    type UserData = ();
    fn event(&mut self, _: &Surface, _: surface::Event, _: &Self::UserData, _: &mut ConnectionHandle, _: &Queue<Self>, _: &mut DataInit<'_>) {
		/*state.size = (scale as u32) * state.unscaled_size;
		surface.set_buffer_scale(scale);
		println!("scale {scale}");
		//state.draw().unwrap()
		state.need_update = true;*/
	}
}

#[path="input.rs"] mod input;

impl<W> Dispatch<WmBase> for State<W> {
    type UserData = ();
    fn event(&mut self, wm_base: &WmBase, event: wm_base::Event, _: &Self::UserData, cx: &mut ConnectionHandle, _: &Queue<Self>, _: &mut DataInit<'_>) {
		if let wm_base::Event::Ping{serial} = event { wm_base.pong(cx, serial); } else { unreachable!() };
    }
}

impl<W> Dispatch<XdgSurface> for State<W> {
    type UserData = ();
    fn event(&mut self, xdg_surface: &XdgSurface, event: xdg_surface::Event, _: &Self::UserData, cx: &mut ConnectionHandle, _: &Queue<Self>, _: &mut DataInit<'_>) {
		if let xdg_surface::Event::Configure{serial, .. } = event {
			/*if unscaled_size.x == 0 || unscaled_size.y == 0 {
				let (scale, size) = with_output_info(env.get_all_outputs().first().unwrap(), |info| (info.scale_factor as u32, ::xy::int2::from(info.modes.first().unwrap().dimensions).into()) ).unwrap();
				let size = vector::component_wise_min(size, widget.size(size));
				assert!(size.x > 0 && size.y > 0 && size.x < 124839, "{:?}", size);
				*unscaled_size = ::xy::div_ceil(size, scale);
			}*/
			dbg!(self.unscaled_size);
			xdg_surface.ack_configure(cx, serial);
			/*let surface = self.surface.as_ref().unwrap();
			let scale = if get_surface_outputs(&surface).is_empty() { // get_surface_outputs defaults to 1 instead of first output factor
				env.get_all_outputs().first().map(|output| with_output_info(output, |info| info.scale_factor)).flatten().unwrap_or(1)
			} else {
				get_surface_scale_factor(&surface)
			};
			eprintln!("configure scale: {scale}");
			*size = (scale as u32) * *unscaled_size;
			surface.set_buffer_scale(scale);
			*need_update = true; // Defer to redraw once*/
		} else { unreachable!() }
    }
}

impl<W> Dispatch<TopLevel> for State<W> {
    type UserData = ();
    fn event(&mut self, _: &TopLevel, event: toplevel::Event, _: &Self::UserData, _: &mut ConnectionHandle, _: &Queue<Self>, _: &mut DataInit<'_>) {
		match event {
			toplevel::Event::Configure{width, height, ..} => { self.unscaled_size=xy{x:width as u32, y:height as u32}; }
        	toplevel::Event::Close => self.xdg_surface = None,
			_ => unreachable!()
        }
    }
}

impl<W:Widget+'static> State<W> {
pub async fn display(&mut self, mut idle: impl FnMut(&mut W)->Result<bool>) -> Result<()> {
	while let Some(event) = std::pin::Pin::new(&mut self.streams).next().await {
		eprintln!("event");
		event(self)?;
		if self.wm_base.is_none() { println!("no wm_base"); break; }
		if idle(&mut self.widget)? { println!("idle"); self.need_update = true; }
		if self.need_update { println!("need_update"); self.draw()? }
	}
	Ok(())
}
#[throws] pub fn draw(&mut self) {
	let Self{widget, display, surface, size, need_update, /*instance, gpu_surface,*/ ..} = self;
	//if *size == (xy{x: 0, y: 0}) { eprintln!("size: 0"); return; }
	eprintln!("draw");
	struct RawWindowHandle(raw_window_handle::RawWindowHandle);
	unsafe impl raw_window_handle::HasRawWindowHandle for RawWindowHandle { fn raw_window_handle(&self) -> raw_window_handle::RawWindowHandle { self.0 } }
	let (instance, gpu_surface) = piet_gpu_hal::Instance::new(Some(&RawWindowHandle(raw_window_handle::RawWindowHandle::Wayland({
		let mut s=raw_window_handle::unix::WaylandHandle::empty(); 
		s.display = display as *mut _ as  *mut _;
		s.surface = surface as *mut _ as  *mut _; 
		s
	}))), Default::default())?;
	let device = unsafe{instance.device(gpu_surface.as_ref())}?;
	let mut swapchain = gpu_surface.map(|surface| unsafe{instance.swapchain(size.x as _, size.y as _, &device, &surface)}).transpose()?;
	let session = piet_gpu_hal::Session::new(device);
	let present_semaphore = unsafe{session.create_semaphore()}?;
	let mut renderer = unsafe{piet_gpu::Renderer::new(&session, size.x as _, size.y as _, 1)}?;
	let mut context = piet_gpu::PietGpuRenderContext::new();
	use piet::RenderContext;
	context.fill(piet::kurbo::Rect::new(0., 0., size.x as _, size.y as _), &piet::Color::WHITE);
	widget.paint(&mut context, *size)?;
	if context.path_count() > 0 {
		renderer.upload_render_ctx(&mut context, 0)?;
	}
	let (image, acquisition_semaphore) = if let Some(swapchain) = swapchain.as_mut() {
		let (image_idx, acquisition_semaphore) = unsafe{swapchain.next()}?;
		(unsafe{swapchain.image(image_idx)}, Some(acquisition_semaphore))
	} else {
		(unsafe{session.create_image2d(size.x, size.y)}?, None)
	};
	let ref query_pool = session.create_query_pool(8)?;
	let mut cmd_buf = session.cmd_buf()?;
	unsafe{
		cmd_buf.begin();
		renderer.record(&mut cmd_buf, &query_pool, 0);
		use piet_gpu_hal::ImageLayout;
		cmd_buf.image_barrier(&image, ImageLayout::Undefined, ImageLayout::BlitDst);
		cmd_buf.blit_image(&renderer.image_dev, &image);
		cmd_buf.image_barrier(&image, ImageLayout::BlitDst, ImageLayout::Present);
		cmd_buf.finish();
		if let Some(swapchain) = swapchain { 
			let submitted = session.run_cmd_buf(cmd_buf, &[&acquisition_semaphore.unwrap()], &[&present_semaphore])?;
			swapchain.present(0, &[&present_semaphore])?;
			submitted.wait()?;
		} else {
			eprintln!("TODO");
		}
	}
	*need_update = false;
	//display.as_ref().map(|d| d.flush().unwrap());*/
}
/*#[throws] pub fn key(&mut self, key: char) -> bool {
	/*let Self{size, modifiers_state, widget, ..} = self;
	if widget.event(*size, &EventContext{modifiers_state, pointer: None}, &Event::Key{key})? { println!("key"); self.need_update = true; true }
	else if key == '⎋' { self.surface=None; false }
	else { false }*/
}*/
pub fn run(mut self, idle: impl FnMut(&mut W)->Result<bool>) -> Result<()> { async_io::block_on(self.display(idle)) }
}
#[throws] pub fn run(widget: impl Widget+'static) { State::new(widget)?.run(|_| Ok(false))? }
