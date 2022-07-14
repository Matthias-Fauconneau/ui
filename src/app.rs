use crate::prelude::*;
use wayland_client::{Connection, Dispatch, QueueHandle as Queue, Proxy, protocol::{
	wl_registry::{self as registry, WlRegistry as Registry},
	wl_compositor::{self as compositor, WlCompositor as Compositor},
	wl_seat::WlSeat as Seat,
	wl_output::{self as output, WlOutput as Output},
	wl_surface::{self as surface, WlSurface as Surface},
	wl_shm::{self as shm, WlShm as Shm},
}};
use wayland_protocols::xdg::shell::client::{
	xdg_wm_base::{self as wm_base, XdgWmBase as WmBase}, xdg_surface::{self, XdgSurface}, xdg_toplevel::{self as toplevel, XdgToplevel as TopLevel}
};
use crate::widget::{Widget, ModifiersState};
use {vector::{xy, size, vec2}, num::{zero, IsZero}};

pub struct State {
	running: bool,
	pub(crate) widget: Box<dyn Widget+'static>,
	wm_base: Option<WmBase>,
	scale: u32,
	configure_bounds: size,
	pub(crate) surface: Option<Surface>,
	//memfd: rustix::io::OwnedFd,
	pub(crate) cursor_surface: Option<Surface>,
	pub(crate) cursor_theme: Option<wayland_cursor::CursorTheme>,
	xdg_surface: Option<(XdgSurface, TopLevel)>,
	unscaled_size: size,
	pub(crate) size: size,
	pub(crate) need_update: bool,
	pub(crate) modifiers_state: ModifiersState,
	pub(crate) cursor_position: vec2,
	pub(crate) mouse_buttons: u32,
	/*instance: piet_gpu_hal::Instance,
	gpu_surface: Option<piet_gpu_hal::Surface>,*/
}

impl State { #[throws] pub fn run(widget: Box<dyn Widget+'static>, idle: &mut dyn FnMut(&mut dyn Widget)->Result<bool>) {
	let connection = Connection::connect_to_env()?;
    let mut event_queue = connection.new_event_queue();
    let ref queue = event_queue.handle();
	let display = connection.display();
    display.get_registry(queue, ())?;
	let ref mut state = Self {
		running: true,
		widget,
		configure_bounds: zero(),
		wm_base: None,
		scale: 3,
		surface: None,
		//memfd: rustix::fs::memfd_create("cursor", rustix::fs::MemfdFlags::empty()).unwrap(),
		cursor_surface: None,
		cursor_theme: None,
		xdg_surface: None,
		unscaled_size: zero(),
		size: zero(),
		need_update: true,
		modifiers_state: ModifiersState::default(),
		cursor_position: zero(),
		mouse_buttons: 0
	};

	while state.wm_base.is_none() || state.surface.is_none() { event_queue.blocking_dispatch(state)?; }
	let (wm_base, surface) = (state.wm_base.as_ref().unwrap(), state.surface.as_ref().unwrap());
	let xdg_surface = wm_base.get_xdg_surface(surface, queue, ()).unwrap();
	let toplevel = xdg_surface.get_toplevel(queue, ()).unwrap();
	toplevel.set_title("App".into());
	surface.commit();
	state.xdg_surface = Some((xdg_surface, toplevel));

	struct RawWindowHandle(raw_window_handle::RawWindowHandle);
	unsafe impl raw_window_handle::HasRawWindowHandle for RawWindowHandle { fn raw_window_handle(&self) -> raw_window_handle::RawWindowHandle { self.0 } }
	let (instance, gpu_surface) = piet_gpu_hal::Instance::new(Some(&RawWindowHandle(raw_window_handle::RawWindowHandle::Wayland({
		let mut s=raw_window_handle::WaylandHandle::empty();
		s.display = display.id().as_ptr() as *mut _;
		s.surface = surface.id().as_ptr() as  *mut _;
		s
	}))), Default::default())?;
	let device = unsafe{instance.device(gpu_surface.as_ref())}?;
	while state.size.is_zero() { event_queue.blocking_dispatch(state)?; }
	let mut swapchain = unsafe{instance.swapchain(state.size.x as _, state.size.y as _, &device, &gpu_surface.unwrap())}?;
	let session = piet_gpu_hal::Session::new(device);
	let present_semaphore = unsafe{session.create_semaphore()}?;
	/*struct RenderContext {
		_instance: piet_gpu_hal::Instance,
		swapchain: piet_gpu_hal::Swapchain,
		session: piet_gpu_hal::Session,
		present_semaphore: piet_gpu_hal::Semaphore,
	}
	render_context = RenderContext{_instance: instance, session, present_semaphore, swapchain};*/

	while state.running {
		let Self{widget, size, need_update, ..} = state;
		if idle(widget.as_mut())? { *need_update = true; dbg!(); }
		if *need_update && !size.is_zero() {
			//let RenderContext{session, swapchain, present_semaphore, ..} = render_context;
			let mut renderer = unsafe{piet_gpu::Renderer::new(&session, size.x as _, size.y as _, 1)}?;
			let mut context = piet_gpu::PietGpuRenderContext::new();
			piet::RenderContext::fill(&mut context, piet::kurbo::Rect::new(0., 0., size.x as _, size.y as _), &piet::Color::WHITE);
			widget.paint(&mut context, *size, num::zero())?;
			/*use piet::{Text, TextLayoutBuilder};
    		let layout = piet::RenderContext::text(&mut context).new_text_layout("Hello World!").default_attribute(piet::TextAttribute::FontSize(size.y as _)).build().unwrap();
    		piet::RenderContext::draw_text(&mut context, &layout, piet::kurbo::Point{x: 0., y: size.y as _});*/
			renderer.upload_render_ctx(&mut context, 0)?;

			let (image_idx, acquisition_semaphore) = unsafe{swapchain.next()}?;
			let image = unsafe{swapchain.image(image_idx)};
			let ref query_pool = session.create_query_pool(12)?;
			let mut cmd_buf = session.cmd_buf()?;
			unsafe{
				cmd_buf.begin();
				renderer.record(&mut cmd_buf, &query_pool, 0);
				use piet_gpu_hal::ImageLayout;
				cmd_buf.image_barrier(&image, ImageLayout::Undefined, ImageLayout::BlitDst);
				cmd_buf.blit_image(&renderer.image_dev, &image);
				cmd_buf.image_barrier(&image, ImageLayout::BlitDst, ImageLayout::Present);
				cmd_buf.finish();
				let submitted = session.run_cmd_buf(cmd_buf, &[&acquisition_semaphore], &[&present_semaphore])?;
				swapchain.present(image_idx, &[&present_semaphore])?;
				submitted.wait()?;
			}
			*need_update = false;
		}
		event_queue.blocking_dispatch(state)?;
	}
}}

impl Dispatch<Registry, ()> for State {
    fn event(Self{wm_base, scale, cursor_theme, surface, cursor_surface, ..}: &mut Self, registry: &Registry, event: registry::Event, _: &(), connection: &Connection, queue: &Queue<Self>) {
		match event {
			registry::Event::Global{name, interface, version, ..} => match &interface[..] {
				"wl_compositor" => {
					let compositor = registry.bind::<Compositor, _, _>(name, version, queue, ()).unwrap();
					*surface = Some(compositor.create_surface(queue, ()).unwrap());
					surface.as_ref().unwrap().set_buffer_scale(*scale as _);
					*cursor_surface = Some(compositor.create_surface(queue, ()).unwrap());
				},
				"wl_seat" => { registry.bind::<Seat, _, _>(name, version, queue, ()).unwrap(); }
				"wl_output" => { registry.bind::<Output, _, _>(name, version, queue, ()).unwrap(); }
				"xdg_wm_base" => *wm_base = Some(registry.bind::<WmBase, _, _>(name, version, queue, ()).unwrap()),
				"wl_shm" => {
                    let shm = registry.bind::<Shm, _, _>(name, 1, queue, ()).unwrap();
                    //let pool = shm.create_pool(connection, self.memfd.as_raw_fd(), (256<<10) as i32, queue, ()).unwrap();
					*cursor_theme = Some(wayland_cursor::CursorTheme::load(connection, shm, 64).unwrap());
                }
				_ => {}
			},
			_ => {}
		};
	}
}

impl Dispatch<Compositor, ()> for State {
    fn event(_: &mut Self, _: &Compositor, _: compositor::Event, _: &(), _: &Connection, _: &Queue<Self>) {}
}

impl Dispatch<Shm, ()> for State {
    fn event(_: &mut Self, _: &Shm, _: shm::Event, _: &(), _: &Connection, _: &Queue<Self>) {}
}

impl Dispatch<Output, ()> for State {
    fn event(Self{scale, surface, ..}: &mut Self, _: &Output, event: output::Event, _: &(), _: &Connection, _: &Queue<Self>) {
		match event {
			//output::Event::Mode{width, height,..} => self.configure_bounds = xy{x: width as _, y: height as _},
			output::Event::Scale{factor} => {
				*scale = factor as _;
				surface.as_ref().unwrap().set_buffer_scale(*scale as _);
			}
			_ => {}
		}
	}
}

impl Dispatch<Surface, ()> for State {
    fn event(_: &mut Self, _: &Surface, _: surface::Event, _: &(), _: &Connection, _: &Queue<Self>) {}
}

impl Dispatch<WmBase, ()> for State {
    fn event(_: &mut Self, wm_base: &WmBase, event: wm_base::Event, _: &(), _: &Connection, _: &Queue<Self>) {
		if let wm_base::Event::Ping{serial} = event { wm_base.pong(serial); } else { unreachable!() };
    }
}

impl Dispatch<XdgSurface, ()> for State {
    fn event(Self{ref scale, configure_bounds, widget, unscaled_size, size, need_update, ..}: &mut Self, xdg_surface: &XdgSurface, event: xdg_surface::Event, _: &(), _: &Connection, _: &Queue<Self>) {
		if let xdg_surface::Event::Configure{serial} = event {
			if unscaled_size.x == 0 || unscaled_size.y == 0 {
				let size = widget.size(*configure_bounds);
				assert!(size <= *configure_bounds);
				*unscaled_size = vector::div_ceil(size, *scale);
			}
			xdg_surface.ack_configure(serial);
			let new_size = *scale * *unscaled_size;
			if new_size != *size {
				*size = new_size;
				*need_update = true;
			}
		} else { unreachable!() }
    }
}

impl Dispatch<TopLevel, ()> for State {
    fn event(Self{configure_bounds, unscaled_size, xdg_surface, ..}: &mut Self, _: &TopLevel, event: toplevel::Event, _: &(), _: &Connection, _: &Queue<Self>) {
		match event {
			toplevel::Event::ConfigureBounds{width, height} => { *configure_bounds=xy{x:width as u32, y:height as u32}; }
			toplevel::Event::Configure{width, height, ..} => { *unscaled_size=xy{x:width as u32, y:height as u32}; }
        	toplevel::Event::Close => *xdg_surface = None,
			_ => panic!("{event:? 	}")
        }
    }
}

#[path="input.rs"] pub mod input;

#[throws] pub fn run(widget: Box<dyn Widget+'static>) { State::run(widget, &mut |_| Ok(false))? }
