use crate::prelude::*;
use wayland_client::{Connection, Dispatch, ConnectionHandle, QueueHandle as Queue, DataInit, Proxy, protocol::{
	wl_registry::{self as registry, WlRegistry as Registry},
	wl_compositor::{self as compositor, WlCompositor as Compositor}, 
	wl_seat::WlSeat as Seat,
	wl_output::{self as output, WlOutput as Output}, 
	wl_surface::{self as surface, WlSurface as Surface}
}};
use wayland_protocols::xdg_shell::client::{
	xdg_wm_base::{self as wm_base, XdgWmBase as WmBase},
	xdg_surface::{self, XdgSurface},
	xdg_toplevel::{self as toplevel, XdgToplevel as TopLevel}
};
use crate::widget::{Widget, ModifiersState};
use {::xy::{xy, size, vec2}, num::{zero, IsZero}};

pub struct State<W> {
	running: bool,
	crate widget: W,
	wm_base: Option<WmBase>,
	scale: u32,
	output: size,
	crate surface: Option<Surface>,
	xdg_surface: Option<(XdgSurface, TopLevel)>,
	unscaled_size: size,
	crate size: size,
	crate need_update: bool,
	crate modifiers_state: ModifiersState,
	crate cursor_position: vec2,
	crate mouse_buttons: u32,
	//instance: piet_gpu_hal::Instance,
	//gpu_surface: Option<piet_gpu_hal::Surface>,
}

impl<W:Widget+'static> State<W> { #[throws] pub fn run(widget: W, mut idle: impl FnMut(&mut W)->Result<bool>) { 
	let cx = Connection::connect_to_env()?;
    let mut event_queue = cx.new_event_queue::<State<W>>();
    let queue_handle = event_queue.handle();
    let display = cx.handle().display();
    display.get_registry(&mut cx.handle(), &queue_handle, ())?;
	let ref mut state = Self {
		running: true,
		widget,
		wm_base: None,
		scale: 1,
		output: zero(),
		surface: None,
		xdg_surface: None,
		unscaled_size: zero(),
		size: zero(),
		need_update: true,
		modifiers_state: ModifiersState::default(),
		cursor_position: zero(),
		mouse_buttons: 0
	};
	while state.running {
		cx.flush()?;
		event_queue.dispatch_pending(state)?;
		let mut fd = [rustix::io::PollFd::from_borrowed_fd(unsafe{rustix::fd::BorrowedFd::borrow_raw_fd(cx.backend().lock().unwrap().connection_fd())}, rustix::io::PollFlags::IN | rustix::io::PollFlags::ERR)];
        loop { match rustix::io::poll(&mut fd, -1) {
            Ok(_) => { break; },
            Err(rustix::io::Error::INTR) => { dbg!(); continue;},
            Err(e) => panic!("{e:?}"),
        }}
		event_queue.blocking_dispatch(state)?;
		let Self{widget, surface, size, need_update, ..} = state;
		if idle(widget)? { *need_update = true; dbg!(); }
		if *need_update && !size.is_zero() {
			struct RawWindowHandle(raw_window_handle::RawWindowHandle);
			unsafe impl raw_window_handle::HasRawWindowHandle for RawWindowHandle { fn raw_window_handle(&self) -> raw_window_handle::RawWindowHandle { self.0 } }
			let (instance, gpu_surface) = piet_gpu_hal::Instance::new(Some(&RawWindowHandle(raw_window_handle::RawWindowHandle::Wayland({
				let mut s=raw_window_handle::unix::WaylandHandle::empty(); 
				s.display = display.id().as_ptr() as *mut wayland_client::protocol::wl_display::WlDisplay as  *mut _;
				s.surface = surface.as_ref().unwrap().id().as_ptr() as *mut Surface as  *mut _; 
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
					unimplemented!()
				}
			}
			*need_update = false;
		}
	}
}}

impl<W: Widget+'static> Dispatch<Registry> for State<W> {
    type UserData = ();
    fn event(&mut self, registry: &Registry, event: registry::Event, _: &Self::UserData, cx: &mut ConnectionHandle, queue: &Queue<Self>, _: &mut DataInit<'_>) {
		match event {
			registry::Event::Global{name, interface, version, ..} => match &interface[..] {
				"wl_compositor" => self.surface = Some(registry.bind::<Compositor, _>(cx, name, version, queue, ()).unwrap().create_surface(cx, queue, ()).unwrap()),
				"wl_seat" => { registry.bind::<Seat, _>(cx, name, version, queue, ()).unwrap(); }
				"wl_output" => { registry.bind::<Output, _>(cx, name, version, queue, ()).unwrap(); }
				"xdg_wm_base" => self.wm_base = Some(registry.bind::<WmBase, _>(cx, name, version, queue, ()).unwrap()),
				_ => {}
			},
			_ => {}
		};
		if self.xdg_surface.is_none() { if let (Some(wm_base), Some(surface)) = (self.wm_base.as_ref(), self.surface.as_ref()) {
			let xdg_surface = wm_base.get_xdg_surface(cx, surface.clone(), queue, ()).unwrap();
			let toplevel = xdg_surface.get_toplevel(cx, queue, ()).unwrap();
			toplevel.set_title(cx, "piet-gpu".into());
			surface.commit(cx);
			self.xdg_surface = Some((xdg_surface, toplevel));
		}}
	}
}

impl<W> Dispatch<Compositor> for State<W> {
    type UserData = ();
    fn event(&mut self, _: &Compositor, _: compositor::Event, _: &Self::UserData, _: &mut ConnectionHandle, _: &Queue<Self>, _: &mut DataInit<'_>) {}
}

impl<W> Dispatch<Output> for State<W> {
    type UserData = ();
    fn event(&mut self, _: &Output, event: output::Event, _: &Self::UserData, cx: &mut ConnectionHandle, _: &Queue<Self>, _: &mut DataInit<'_>) { 
		match event {
			output::Event::Mode{width, height,..} => self.output = xy{x: width as _, y: height as _},
			output::Event::Scale{factor} => {
				self.scale = factor as _;
				self.surface.as_ref().unwrap().set_buffer_scale(cx, self.scale as _);
			}
			_ => {}
		}
	}
}

impl<W> Dispatch<Surface> for State<W> {
    type UserData = ();
    fn event(&mut self, _: &Surface, _: surface::Event, _: &Self::UserData, _: &mut ConnectionHandle, _: &Queue<Self>, _: &mut DataInit<'_>) {}
}

impl<W> Dispatch<WmBase> for State<W> {
    type UserData = ();
    fn event(&mut self, wm_base: &WmBase, event: wm_base::Event, _: &Self::UserData, cx: &mut ConnectionHandle, _: &Queue<Self>, _: &mut DataInit<'_>) {
		if let wm_base::Event::Ping{serial} = event { wm_base.pong(cx, serial); } else { unreachable!() };
    }
}

impl<W:Widget> Dispatch<XdgSurface> for State<W> {
    type UserData = ();
    fn event(&mut self, xdg_surface: &XdgSurface, event: xdg_surface::Event, _: &Self::UserData, cx: &mut ConnectionHandle, _: &Queue<Self>, _: &mut DataInit<'_>) {
		if let xdg_surface::Event::Configure{serial} = event {
			let Self{ref scale, output, widget, unscaled_size, size, need_update, ..} = self;
			if unscaled_size.x == 0 || unscaled_size.y == 0 {
				let size = widget.size(*output);
				assert!(size <= *output);
				*unscaled_size = ::xy::div_ceil(size, *scale);
			}
			xdg_surface.ack_configure(cx, serial);
			let new_size = *scale * *unscaled_size;
			if new_size != *size {
				*size = new_size;
				*need_update = true;
			}
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

#[path="input.rs"] mod input;

#[throws] pub fn run(widget: impl Widget+'static) { State::run(widget, |_| Ok(false))? }
