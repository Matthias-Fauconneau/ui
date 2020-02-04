use {std::{cell::{Cell, RefCell}, rc::Rc}, crate::{core::Result, vector::size2, image::{Image, bgra8, IntoImage}}};

pub type Target<'t> = Image<&'t mut[bgra8]>;
pub trait Widget {
    fn size(&mut self, size : size2) -> size2 { size }
    fn render(&mut self, target : &mut Target) -> Result;
}

pub fn run<W:Widget+'static>(widget:W) -> Result { run_rc(Rc::new(RefCell::new(widget))) }
pub fn run_rc(widget : Rc<RefCell<dyn Widget>>) -> Result {
    std::panic::set_hook(Box::new(|info| {
        println!("{}: {}", info.location().unwrap(), match info.payload().downcast_ref::<&'static str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => &s[..], // Temporary reference => no map closure => match
                None => "Box<Any>",
            }
        });
    }));

    let display = wayland_client::Display::connect_to_env()?;
    let mut event_queue = display.create_event_queue();
    let attached_display = (*display).clone().attach(event_queue.get_token());
    let globals = wayland_client::GlobalManager::new(&attached_display);
    event_queue.sync_roundtrip(|_, _| unreachable!())?;
    let compositor = globals.instantiate_range::<wayland_client::protocol::wl_compositor::WlCompositor>(1, 4)?;
    let surface = compositor.create_surface();
    surface.set_buffer_scale(3);
    let shm = globals.instantiate_exact::<wayland_client::protocol::wl_shm::WlShm>(1).expect("shm");
    use wayland_protocols::wlr::unstable::layer_shell::v1::client::{zwlr_layer_shell_v1::{ZwlrLayerShellV1, Layer}, zwlr_layer_surface_v1::Event};
    let layer_surface = globals.instantiate_range::<ZwlrLayerShellV1>(1, 2)?.get_layer_surface(&surface, None, Layer::Overlay, "status".to_string());
    layer_surface.set_keyboard_interactivity(1);
    surface.commit();

    let exit = Rc::new(Cell::new(false));
    layer_surface.assign_mono({/*let exit = Rc:: downgrade(&exit);*/ move |layer_surface, event| { match event {
            Event::Closed => {},
            Event::Configure {serial, width, height} => {
                if !(width > 0 && height > 0) {
                    let size = widget.borrow_mut().size(size2{x:3840, y:2160});
                    layer_surface.set_size(size.x/3, size.y/3);
                    layer_surface.ack_configure(serial);
                    surface.commit();
                    return;
                }
                let size = size2{x:width*3, y:height*3};
                let file = tempfile::tempfile().unwrap();
                file.set_len((size.x*size.y*4) as u64).unwrap();
                let bytes = &mut unsafe{memmap::MmapMut::map_mut(&file)}.unwrap()[..]; // ? unmap on drop
                let mut target = unsafe{std::slice::from_raw_parts_mut(bytes.as_mut_ptr() as *mut bgra8, bytes.len() / std::mem::size_of::<bgra8>())}.image(size);
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{widget.borrow_mut().render(&mut target).unwrap_or_else(|e|println!("{:?}",e))})).unwrap_or_else(|_|{});
                // Exiting before complete setup seems to crash sway soon after :/ Show partial render instead
                // FIXME: reuse pool+buffer
                let pool = shm.create_pool((&file as &dyn std::os::unix::io::AsRawFd).as_raw_fd(), (target.size.x*target.size.y*4) as i32);
                let buffer = pool.create_buffer(0, target.size.x as i32, target.size.y as i32, (target.stride*4) as i32, wayland_client::protocol::wl_shm::Format::Argb8888);
                surface.attach(Some(&buffer), 0, 0);
                layer_surface.ack_configure(serial);
                surface.commit();
            },
            _ => unreachable!(),
    }}});
    globals.instantiate_range::<wayland_client::protocol::wl_seat::WlSeat>(1, 7)?.assign_mono({let exit = Rc:: downgrade(&exit); move |seat, event| { match event {
        wayland_client::protocol::wl_seat::Event::Name { .. } => {}
        wayland_client::protocol::wl_seat::Event::Capabilities { capabilities } => {
            if capabilities.contains(wayland_client::protocol::wl_seat::Capability::Keyboard) { seat.get_keyboard().assign_mono({let exit = exit.clone(); move |_, event|
            match event {
                wayland_client::protocol::wl_keyboard::Event::Keymap { .. } => {}
                wayland_client::protocol::wl_keyboard::Event::Enter { .. } => {}
                wayland_client::protocol::wl_keyboard::Event::Leave { .. } => {}
                wayland_client::protocol::wl_keyboard::Event::Key { /*key, state,*/ .. } => { (||->Option<()>{Some(exit.upgrade()?.set(true))})(); /*try_set(&exit);*/ }
                wayland_client::protocol::wl_keyboard::Event::Modifiers { .. } => {}
                wayland_client::protocol::wl_keyboard::Event::RepeatInfo { .. } => {}
                _ => { unreachable!() }
            }});}
            if capabilities.contains(wayland_client::protocol::wl_seat::Capability::Pointer) { seat.get_pointer().assign_mono({let exit = exit.clone(); move |_, event|
            match event {
                wayland_client::protocol::wl_pointer::Event::Enter { /*surface_x, surface_y,*/ .. } => {}
                wayland_client::protocol::wl_pointer::Event::Leave { .. } => { (||->Option<()>{Some(exit.upgrade()?.set(true))})(); }
                wayland_client::protocol::wl_pointer::Event::Motion { /*surface_x, surface_y,*/ .. } => {}
                wayland_client::protocol::wl_pointer::Event::Button { /*button, state,*/ .. } => {}
                _ => {}
            }});}
        }
        _ => unreachable!(),
    }}});
    event_queue.sync_roundtrip(|_, _|{})?;
    while !exit.get() { event_queue.dispatch(|_, _|{})?; }
    Ok(())
}
