use {std::{cell::{Cell}, rc::Rc}, crate::{core::Result, vector::size2, image::{Image, bgra8, IntoImage}}};

pub type Target<'t> = Image<&'t mut[bgra8]>;
pub trait Widget {
    fn size(&mut self, size : size2) -> size2 { size }
    fn render(&mut self, target : &mut Target);
}

//pub fn run<W:Widget>(widget:W) -> Result { run_box_dyn(box widget) }
//pub fn run_box_dyn(mut widget : Box<dyn Widget>) -> Result {
pub fn run(widget: &dyn Widget) -> Result {
    use wayland_client::{Display, GlobalManager, event_enum, Filter,
        protocol::{wl_seat as seat, wl_keyboard as keyboard, wl_pointer as pointer, wl_compositor as compositor, wl_shm as shm, wl_buffer as buffer}};
    let display = Display::connect_to_env()?;
    let mut event_queue = display.create_event_queue();
    let globals = GlobalManager::new(&(*display).clone().attach(event_queue.token()));
    event_queue.sync_roundtrip(&mut(), |_,_,_| unreachable!())?;
    let compositor = globals.instantiate_range::<compositor::WlCompositor>(1, 4)?;
    let surface = compositor.create_surface();
    surface.set_buffer_scale(3);
    let shm = globals.instantiate_exact::<shm::WlShm>(1).expect("shm");
    use wayland_protocols::wlr::unstable::layer_shell::v1::client::{zwlr_layer_shell_v1::{ZwlrLayerShellV1, Layer}, zwlr_layer_surface_v1 as layer_surface};
    let layer_surface = globals.instantiate_range::<ZwlrLayerShellV1>(1, 2)?.get_layer_surface(&surface, None, Layer::Overlay, "status".to_string());
    layer_surface.set_keyboard_interactivity(1);
    surface.commit();

    struct State<'t, W> {widget: &'t W, exit: Cell<bool>}
    let state = Rc::new(State{widget: &widget, exit: Cell::new(false)});
    let filter = Filter::new({
        let state = Rc::downgrade(&state);

        rental! { mod rent {
            #[rental(covariant)]
            pub struct MapTarget {
                map: Box<memmap::MmapMut>,
                target: super::Target<'map>
            }
        } } use rent::MapTarget;

        let buffer = None;
        move |event, filter, _| {
            event_enum!(Event | LayerSurface => layer_surface::ZwlrLayerSurfaceV1, Buffer => buffer::WlBuffer, Seat => seat::WlSeat,
                                              Pointer => pointer::WlPointer, Keyboard => keyboard::WlKeyboard);
            use Event::*;
            match event {
                LayerSurface{event:layer_surface::Event::Closed, ..} => {},
                LayerSurface{event:layer_surface::Event::Configure{serial, width, height}, object:layer_surface} => {
                    let widget = state.upgrade().unwrap().widget;
                    if !(width > 0 && height > 0) {
                        let size = widget.size(size2{x:3840, y:2160});
                        layer_surface.set_size(size.x/3, size.y/3);
                        layer_surface.ack_configure(serial);
                        surface.commit();
                        return;
                    }
                    layer_surface.ack_configure(serial);
                    buffer = {
                        let file = tempfile::tempfile().unwrap();
                        let size = size2{x:width*3, y:height*3};
                        let pool = {
                            let size = (size.x*size.y*4) as u64;
                            file.set_len(size).unwrap();
                            shm.create_pool((&file as &dyn std::os::unix::io::AsRawFd).as_raw_fd(), size as i32)
                        };
                        let target = MapTarget::new(box unsafe{memmap::MmapMut::map_mut(&file)}.unwrap(),
                                                            |map| unsafe{std::slice::from_raw_parts_mut(map.as_mut_ptr() as *mut bgra8, map.len() / std::mem::size_of::<bgra8>())}.image(size));
                        Some((
                            {let target = target.suffix();
                                let buffer = pool.create_buffer(0, target.size.x as i32, target.size.y as i32, (target.stride*4) as i32, shm::Format::Argb8888);
                                buffer.assign(filter.clone());

                                widget.render(&mut target);
                                surface.attach(Some(&buffer), 0, 0);
                                surface.commit();
                            buffer},
                            target))
                    }
                },
                LayerSurface{..} => panic!("LayerSurface"),
                Buffer{event:buffer::Event::Release, object:_buffer} => {},
                    // assert(_buffer==buffer) widget.render_attach_commit(&mut target.as_mut().unwrap().1, &surface, &buffer); },
                Buffer{..} => panic!("Buffer"),
                Seat{event:seat::Event::Name{..}, ..} => {},
                Seat{event:seat::Event::Capabilities{capabilities}, object:seat} => {
                    if capabilities.contains(seat::Capability::Keyboard) { seat.get_keyboard().assign(filter.clone()); }
                    if capabilities.contains(seat::Capability::Pointer) { seat.get_pointer().assign(filter.clone()); }
                },
                Seat{..} => panic!("Seat"),
                Keyboard{event:keyboard::Event::Keymap{..}, ..} => {},
                Keyboard{event:keyboard::Event::Enter{..}, ..} => {},
                Keyboard{event:keyboard::Event::Leave{..}, ..} => {},
                Keyboard{event:keyboard::Event::Key{/*key,state,*/..}, ..} => { (||->Option<()>{Some(state.upgrade()?.exit.set(true))})(); },
                Keyboard{event:keyboard::Event::Modifiers{/*key,state,*/..}, ..} => {},
                Keyboard{event:keyboard::Event::RepeatInfo{/*key,state,*/..}, ..} => {},
                Keyboard{..} => panic!("Keyboard"),
                Pointer{event:pointer::Event::Enter{/*surface_x,surface_y,*/..}, ..} => {},
                Pointer{event:pointer::Event::Leave{..}, ..} => { (||->Option<()>{Some(state.upgrade()?.exit.set(true))})(); },
                Pointer{event:pointer::Event::Motion{/*surface_x,surface_y,*/..}, ..} => {},
                Pointer{event:pointer::Event::Button{/*button,state,*/..}, ..} => {},
                Pointer{event:pointer::Event::Axis{..}, ..} => {},
                Pointer{event:pointer::Event::Frame{..}, ..} => {},
                Pointer{event:pointer::Event::AxisSource{..}, ..} => {},
                Pointer{event:pointer::Event::AxisStop{..}, ..} => {},
                Pointer{event:pointer::Event::AxisDiscrete{..}, ..} => {},
                Pointer{..} => panic!("Pointer"),
            }
        }
    });
    layer_surface.assign(filter.clone());
    globals.instantiate_range::<seat::WlSeat>(1, 7)?.assign(filter);
    while !state.exit.get() { event_queue.dispatch(&mut(), |_,_,_|{})?; }
    let state = Rc::try_unwrap(state).unwrap_or_else(|_|panic!());
    Ok(())
}
