#!/usr/bin/fish -c cargo watch -s 'cargo +nightly run'
//--release --color always 2>&1 | bat -p --paging always'
#![feature(type_ascription,try_blocks)]

fn main() -> anyhow::Result<()> {
    let display = wayland_client::Display::connect_to_env()?;
    let mut event_queue = display.create_event_queue();
    let attached_display = (*display).clone().attach(event_queue.get_token());
    let globals = wayland_client::GlobalManager::new(&attached_display);
    event_queue.sync_roundtrip(|_, _| unreachable!())?;
    let compositor = globals.instantiate_range::<wayland_client::protocol::wl_compositor::WlCompositor>(1, 4)?;
    let surface = compositor.create_surface();
    surface.set_buffer_scale(3);
    let shm = globals.instantiate_exact::<wayland_client::protocol::wl_shm::WlShm>(1).unwrap();
    use wayland_protocols::wlr::unstable::layer_shell::v1::client::{zwlr_layer_shell_v1::{ZwlrLayerShellV1, Layer}, zwlr_layer_surface_v1::Event};
    let layer_surface = globals.instantiate_range::<ZwlrLayerShellV1>(1, 2)?.get_layer_surface(&surface, None, Layer::Overlay, "status".to_string());
    layer_surface.set_keyboard_interactivity(1);
    surface.commit();
    layer_surface.assign_mono(move |layer_surface, event| { match event {
            Event::Closed => {},
            Event::Configure {serial, width, height} => {
                if !(width > 0 && height > 0) { layer_surface.set_size(3840/3, 2160/3/3); layer_surface.ack_configure(serial); surface.commit(); return; }
                let (width, height) = (width*3, height*3);
                let file = tempfile::tempfile().unwrap();
                file.set_len((width*height*4) as u64).unwrap();
                trait Fill<T: Copy> { fn fill(&mut self, v: T); }
                impl<T: Copy> Fill<T> for [T] { fn fill(&mut self, v: T) { for e in self.iter_mut() { *e = v; } } }
                unsafe{memmap::MmapMut::map_mut(&file)}.unwrap().fill(0xFF);
                // FIXME: reuse pool+buffer
                let pool = shm.create_pool((&file as &dyn std::os::unix::io::AsRawFd).as_raw_fd(), (width*height*4) as i32);
                let buffer = pool.create_buffer(0, width as i32, height as i32, (width*4) as i32, wayland_client::protocol::wl_shm::Format::Argb8888);
                surface.attach(Some(&buffer), 0, 0);
                layer_surface.ack_configure(serial);
                surface.commit();
            },
            _ => unreachable!(),
    }});
    use std::rc::Rc;
    let exit = Rc::new(std::cell::Cell::new(false));
    globals.instantiate_range::<wayland_client::protocol::wl_seat::WlSeat>(1, 7)?.assign_mono({let exit = Rc:: downgrade(&exit); move |seat, event| { match event {
        wayland_client::protocol::wl_seat::Event::Name { .. } => {}
        wayland_client::protocol::wl_seat::Event::Capabilities { capabilities } => {
            if capabilities.contains(wayland_client::protocol::wl_seat::Capability::Keyboard) { seat.get_keyboard().assign_mono({let exit = exit.clone(); move |_, event|
            match event {
                wayland_client::protocol::wl_keyboard::Event::Keymap { .. } => {}
                wayland_client::protocol::wl_keyboard::Event::Enter { .. } => {}
                wayland_client::protocol::wl_keyboard::Event::Leave { .. } => { }
                wayland_client::protocol::wl_keyboard::Event::Key { key, state, .. } => {
                    println!("Key with id {} was {:?}.", key, state);
                    (try { exit.upgrade()?.set(true); }) : Option<_>;
                }
                wayland_client::protocol::wl_keyboard::Event::Modifiers { .. } => {}
                wayland_client::protocol::wl_keyboard::Event::RepeatInfo { .. } => {}
                _ => { unreachable!() }
            }});}
            if capabilities.contains(wayland_client::protocol::wl_seat::Capability::Pointer) { seat.get_pointer().assign_mono({let exit = exit.clone(); move |_, event|
            match event {
                wayland_client::protocol::wl_pointer::Event::Enter { surface_x, surface_y, .. } => { println!("Pointer entered at ({}, {}).", surface_x, surface_y); }
                wayland_client::protocol::wl_pointer::Event::Leave { .. } => {
                    println!("Pointer left.");
                    (try { exit.upgrade()?.set(true); }) : Option<_>;
                }
                wayland_client::protocol::wl_pointer::Event::Motion { /*surface_x, surface_y,*/ .. } => { /*println!("Pointer moved to ({}, {}).", surface_x, surface_y);*/ }
                wayland_client::protocol::wl_pointer::Event::Button { button, state, .. } => { println!("Button {} was {:?}.", button, state); }
                _ => {}
            }});}
        }
        _ => unreachable!(),
    }}});
    event_queue.sync_roundtrip(|_, _|{})?;
    while !exit.get() { event_queue.dispatch(|_, _|{})?; }
    Ok(())
}
