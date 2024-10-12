use {vector::num::zero, crate::{Event, EventContext, Widget}, softbuffer::{Context, Surface}, std::rc::Rc, vector::xy, winit::{application::ApplicationHandler, event::{ElementState, KeyEvent, WindowEvent::{self, *}}, event_loop::{ActiveEventLoop, EventLoop}, keyboard::{Key::{Character, Named}, NamedKey::Escape}, window::{Window, WindowId}}};
pub struct App<'t, T> {
	widget: &'t mut T,
	event_context: EventContext,
	surface: Option<Surface<Rc<Window>, Rc<Window>>>
}
impl<T: Widget> ApplicationHandler for App<'_, T> {
	fn resumed(&mut self, event_loop: &ActiveEventLoop) {
		let window = Rc::new(event_loop.create_window(Default::default()).unwrap());
		self.surface = Some(Surface::new(&Context::new(window.clone()).unwrap(), window).unwrap());
	}
	fn suspended(&mut self, _: &ActiveEventLoop) {}
	fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
		let (widget, event_context, surface) = (&mut self.widget, &mut self.event_context, self.surface.as_mut().unwrap());
		let size = {let size = surface.window().inner_size(); xy{x: size.width, y: size.height}};
		let scale_factor = surface.window().scale_factor();
		let mut redraw = || {
			widget.event(size, event_context, &Event::Stale).unwrap();
			surface.resize(std::num::NonZeroU32::new(size.x).unwrap(), std::num::NonZeroU32::new(size.y).unwrap()).unwrap();
			let mut buffer = surface.buffer_mut().unwrap();
			let mut target = image::Image::new::<u32>(size, &mut *buffer);
			target.data.fill(image::bgr8::from(crate::background()).into());
			widget.paint(&mut target, size, zero()).unwrap();
			buffer.present().unwrap();
		};
		match event {
			ScaleFactorChanged{scale_factor, ..} => { dbg!(scale_factor); redraw(); }
			RedrawRequested => if scale_factor != 3. {
				surface.resize(std::num::NonZeroU32::new(size.x).unwrap(), std::num::NonZeroU32::new(size.y).unwrap()).unwrap();
				surface.buffer_mut().unwrap().present().unwrap();
			} else { redraw() }
			CloseRequested|KeyboardInput{event:KeyEvent{logical_key:Named(Escape), ..},..} => event_loop.exit(),
			KeyboardInput{event: KeyEvent{logical_key: Character(c), state:ElementState::Pressed, ..},..} =>
				if widget.event(size, event_context, &Event::Key(c.chars().next().unwrap())).unwrap() { surface.window().request_redraw(); },
			_ => {}
		}
	}
	fn about_to_wait(&mut self, _: &winit::event_loop::ActiveEventLoop) {}
}
pub fn run<T:Widget>(_title: &str, widget: &mut T) {  
	EventLoop::new().unwrap().run_app(&mut App{widget, event_context: EventContext{modifiers_state: Default::default()}, surface: None}).unwrap();
}