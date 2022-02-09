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
use {vector::{xy, size, vec2}, num::{zero, IsZero}};

pub struct State {
	running: bool,
	crate widget: Box<dyn Widget+'static>,
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

impl State { #[throws] pub fn run(widget: Box<dyn Widget+'static>, idle: &mut dyn FnMut(&mut dyn Widget)->Result<bool>) {
	let connection = Connection::connect_to_env()?;
    let mut event_queue = connection.new_event_queue();
    let ref queue = event_queue.handle();
	let display = {
		let ref mut cx = connection.handle();
    	let display = cx.display();
    	display.get_registry(cx, queue, ())?;
		display
	};
	let ref mut state = Self {
		running: true,
		widget,
		output: zero(),
		wm_base: None,
		scale: 1,
		surface: None,
		xdg_surface: None,
		unscaled_size: zero(),
		size: zero(),
		need_update: true,
		modifiers_state: ModifiersState::default(),
		cursor_position: zero(),
		mouse_buttons: 0
	};

	struct RenderContext {
		_instance: piet_gpu_hal::Instance,
		swapchain: Option<piet_gpu_hal::Swapchain>,
		session: piet_gpu_hal::Session,
		present_semaphore: piet_gpu_hal::Semaphore,
		target: image::Image<Box<[image::bgra8]>>,
		buffer: piet_gpu_hal::Buffer
	}
	let mut render_context = None;

	while state.running {
		/*connection.flush()?;
		event_queue.dispatch_pending(state)?;
		let mut fd = [rustix::io::PollFd::from_borrowed_fd(unsafe{rustix::fd::BorrowedFd::borrow_raw_fd(connection.backend().lock().unwrap().connection_fd())}, rustix::io::PollFlags::IN | rustix::io::PollFlags::ERR)];
        loop { match rustix::io::poll(&mut fd, -1) {
            Ok(_) => { break; },
            Err(rustix::io::Error::INTR) => { dbg!(); continue;},
            Err(e) => panic!("{e:?}"),
        }}*/
		event_queue.blocking_dispatch(state)?;
		let Self{widget, surface, size, need_update, ..} = state;
		if idle(widget.as_mut())? { *need_update = true; dbg!(); }
		if state.xdg_surface.is_none() { if let (false, Some(wm_base), Some(surface)) = (state.output.is_zero(), state.wm_base.as_ref(), &surface) {
			let ref mut cx = connection.handle();
			let xdg_surface = wm_base.get_xdg_surface(cx, surface.clone(), queue, ()).unwrap();
			let toplevel = xdg_surface.get_toplevel(cx, queue, ()).unwrap();
			toplevel.set_title(cx, "piet-gpu".into());
			surface.commit(cx);
			state.xdg_surface = Some((xdg_surface, toplevel));
		}}
		if *need_update && !size.is_zero() {
			use num::Zero;
			if render_context.as_ref().map(|r:&RenderContext| r.target.size).unwrap_or(size::ZERO) != *size {
				struct RawWindowHandle(raw_window_handle::RawWindowHandle);
				unsafe impl raw_window_handle::HasRawWindowHandle for RawWindowHandle { fn raw_window_handle(&self) -> raw_window_handle::RawWindowHandle { self.0 } }
				let (instance, gpu_surface) = piet_gpu_hal::Instance::new(Some(&RawWindowHandle(raw_window_handle::RawWindowHandle::Wayland({
					let mut s=raw_window_handle::WaylandHandle::empty();
					s.display = display.id().as_ptr() as *mut wayland_client::protocol::wl_display::WlDisplay as  *mut _;
					s.surface = surface.as_ref().unwrap().id().as_ptr() as *mut Surface as  *mut _;
					s
				}))), Default::default())?;
				let device = unsafe{instance.device(gpu_surface.as_ref())}?;
				let swapchain = gpu_surface.map(|surface| unsafe{instance.swapchain(size.x as _, size.y as _, &device, &surface)}).transpose()?;
				let session = piet_gpu_hal::Session::new(device);
				let present_semaphore = unsafe{session.create_semaphore()}?;
				let target = image::Image::uninitialized(*size);
				let buffer = session.create_buffer((target.len() * 4) as u64, piet_gpu_hal::BufferUsage::MAP_WRITE|piet_gpu_hal::BufferUsage::COPY_SRC)?;
				render_context = Some(RenderContext{_instance: instance, session, present_semaphore, target, buffer, swapchain});
			}
			let RenderContext{session, target, buffer, swapchain, present_semaphore, ..} = render_context.as_mut().unwrap();
			widget.paint(&mut target.as_mut(), *size)?;
			unsafe{buffer.write(&target)?};

			let (image_idx, image, acquisition_semaphore) = if let Some(swapchain) = swapchain.as_mut() {
				let (image_idx, acquisition_semaphore) = unsafe{swapchain.next()}?;
				(image_idx, unsafe{swapchain.image(image_idx)}, Some(acquisition_semaphore))
			} else {
				(0, unsafe{session.create_image2d(size.x, size.y)}?, None)
			};
			let mut cmd_buf = session.cmd_buf()?;
			unsafe{
				cmd_buf.begin();
				use piet_gpu_hal::ImageLayout;
				cmd_buf.image_barrier(&image, ImageLayout::Undefined, ImageLayout::BlitDst);
				cmd_buf.copy_buffer_to_image(&buffer, &image);
				cmd_buf.image_barrier(&image, ImageLayout::BlitDst, ImageLayout::Present);
				cmd_buf.finish();
				if let Some(swapchain) = swapchain.as_mut() {
					let submitted = session.run_cmd_buf(cmd_buf, &[&acquisition_semaphore.unwrap()], &[&present_semaphore])?;
					swapchain.present(image_idx, &[&present_semaphore])?;
					submitted.wait()?;
				} else {
					unimplemented!()
				}
			}
			*need_update = false;
		}
	}
}}

impl Dispatch<Registry> for State {
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
	}
}

impl Dispatch<Compositor> for State {
    type UserData = ();
    fn event(&mut self, _: &Compositor, _: compositor::Event, _: &Self::UserData, _: &mut ConnectionHandle, _: &Queue<Self>, _: &mut DataInit<'_>) {}
}

impl Dispatch<Output> for State {
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

impl Dispatch<Surface> for State {
    type UserData = ();
    fn event(&mut self, _: &Surface, _: surface::Event, _: &Self::UserData, _: &mut ConnectionHandle, _: &Queue<Self>, _: &mut DataInit<'_>) {}
}

impl Dispatch<WmBase> for State {
    type UserData = ();
    fn event(&mut self, wm_base: &WmBase, event: wm_base::Event, _: &Self::UserData, cx: &mut ConnectionHandle, _: &Queue<Self>, _: &mut DataInit<'_>) {
		if let wm_base::Event::Ping{serial} = event { wm_base.pong(cx, serial); } else { unreachable!() };
    }
}

impl Dispatch<XdgSurface> for State {
    type UserData = ();
    fn event(&mut self, xdg_surface: &XdgSurface, event: xdg_surface::Event, _: &Self::UserData, cx: &mut ConnectionHandle, _: &Queue<Self>, _: &mut DataInit<'_>) {
		if let xdg_surface::Event::Configure{serial} = event {
			let Self{ref scale, output, widget, unscaled_size, size, need_update, ..} = self;
			if unscaled_size.x == 0 || unscaled_size.y == 0 {
				let size = widget.size(*output);
				assert!(size <= *output);
				*unscaled_size = vector::div_ceil(size, *scale);
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

impl Dispatch<TopLevel> for State {
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

#[throws] pub fn run(widget: Box<dyn Widget+'static>) { State::run(widget, &mut |_| Ok(false))? }
