#![feature(non_ascii_idents)]
mod core; use crate::core::{Result};
mod image; use image::{size2, bgra8, IntoImage};
mod raster;
mod text; use text::{ceil_div, Font, metrics, render};

fn try_set(flag : &std::rc::Weak<std::cell::Cell<bool>>) -> Option<()> { Some(flag.upgrade()?.set(true)) }

fn main() -> Result {
    std::panic::set_hook(Box::new(|info| {
        println!("{}: {}", info.location().unwrap(), match info.payload().downcast_ref::<&'static str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => &s[..], // Temporary reference => no map closure => match
                None => "Box<Any>",
            }
        });
    }));

    let font = Rc::new(Font::map()?);

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

    use std::rc::Rc;
    let exit = Rc::new(std::cell::Cell::new(false));
    layer_surface.assign_mono({/*let exit = Rc:: downgrade(&exit);*/ move |layer_surface, event| { match event {
            Event::Closed => {},
            Event::Configure {serial, width, height} => {
                let text = "orOR"; //Hello World! ⅞";
                if !(width > 0 && height > 0) {
                    let metrics = metrics(&*font, text).unwrap();
                    layer_surface.set_size(3840/3, (ceil_div(3840*(metrics.height()-1) as u32, metrics.width)+1)/3);
                    layer_surface.ack_configure(serial);
                    surface.commit();
                    return;
                }
                let size = size2::from((width*3, height*3));
                let file = tempfile::tempfile().unwrap();
                file.set_len((size.x*size.y*4) as u64).unwrap();
                let bytes = &mut unsafe{memmap::MmapMut::map_mut(&file)}.unwrap()[..]; // ? unmap on drop
                let mut target = unsafe{std::slice::from_raw_parts_mut(bytes.as_mut_ptr() as *mut bgra8, bytes.len() / std::mem::size_of::<bgra8>())}.image(size).unwrap();
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{ render(&font, &mut target, "orOR").unwrap_or_else(|e| println!("{}", e)) })).unwrap_or_else(|_|{ /*try_set(&exit);*/ });
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
                wayland_client::protocol::wl_keyboard::Event::Leave { .. } => { try_set(&exit); }
                wayland_client::protocol::wl_keyboard::Event::Key { /*key, state,*/ .. } => { try_set(&exit); }
                wayland_client::protocol::wl_keyboard::Event::Modifiers { .. } => {}
                wayland_client::protocol::wl_keyboard::Event::RepeatInfo { .. } => {}
                _ => { unreachable!() }
            }});}
            if capabilities.contains(wayland_client::protocol::wl_seat::Capability::Pointer) { seat.get_pointer().assign_mono({let exit = exit.clone(); move |_, event|
            match event {
                wayland_client::protocol::wl_pointer::Event::Enter { /*surface_x, surface_y,*/ .. } => {}
                wayland_client::protocol::wl_pointer::Event::Leave { .. } => { try_set(&exit); }
                wayland_client::protocol::wl_pointer::Event::Motion { /*surface_x, surface_y,*/ .. } => {}
                wayland_client::protocol::wl_pointer::Event::Button { /*button, state,*/ .. } => {}
                _ => {}
            }});}
        }
        _ => unreachable!(),
    }}});
    event_queue.sync_roundtrip(|_, _|{})?;
    while { event_queue.dispatch(|_, _|{})?; !exit.get() } {}
    Ok(())
}
