#![feature(type_ascription,try_blocks)]
mod image;
use image::{uint2, size2, Image, bgra8, IntoImage, IntoPixelIterator};

#[allow(non_camel_case_types)] struct vec2(f32, f32);
impl From<(f32, f32)> for vec2 { fn from(v: (f32, f32)) -> Self { vec2(v.0, v.1) } }
impl std::ops::Mul<f32> for vec2 { type Output=Self; fn mul(self, b: f32) -> Self { vec2(self.0*b, self.1*b) } }
use std::convert::TryInto;
impl std::convert::TryFrom<vec2> for uint2 {
    type Error = <u32 as std::convert::TryFrom<i32>>::Error;
    fn try_from(v: vec2) -> Result<Self, Self::Error> { Ok(uint2{x:(v.0 as i32).try_into()?, y:(v.1 as i32).try_into()?}) }
}

trait OrPrint<T> { fn or_print<C:std::fmt::Debug>(self, _:C) -> Option<T>; }
impl<T> OrPrint<T> for Option<T> { fn or_print<C:std::fmt::Debug>(self, c:C) -> Option<T> { self.or_else(|| { println!("{:?}", c); None}) } }

fn render(target : &mut Image<&mut[bgra8]>) -> anyhow::Result<()> {
    let font_data = unsafe{memmap::Mmap::map(&std::fs::File::open("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")?)}?;
    let font = fontdue::Font::from_bytes(font_data).map_err(|e|anyhow::anyhow!(e))?; //.unwrap(); // hash, //path
    let font_data = unsafe{memmap::Mmap::map(&std::fs::File::open("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")?)}?;
    use anyhow::Context;
    let stb = stb_truetype::FontInfo::new(&*font_data, 0).context("stb")?; // FIXME: (kern|ttf-parser)/fontdue
    let text = "Hello World! ⅞";
    let (text_width, text_line_ascent, _, _) = text.chars().fold((0, 0, 0, None), |(_, text_line_ascent, mut pen, last_glyph_index), character| {
        let glyph_index = font.glyph_index(character);
        let glyph = font.glyph(glyph_index);

        let stb_glyph_index = stb.find_glyph_index(character as u32);
        assert_eq!(glyph_index, stb_glyph_index);

        if let Some(last_glyph_index) = last_glyph_index { pen += stb.get_glyph_kern_advance(last_glyph_index, stb_glyph_index); }
        let metrics = stb.get_glyph_h_metrics(stb_glyph_index);
        let width = (try {
            let rect = stb.get_glyph_box(stb_glyph_index)?;
            rect.x1-rect.x0
        } : Option<_>) .unwrap_or(0);
        let x = pen + metrics.left_side_bearing + width as i32;
        (x, std::cmp::max(text_line_ascent, glyph.ascent), pen+metrics.advance_width, Some(stb_glyph_index))
    });
    let scale = target.size.x as f32 / text_width as f32;
    text.chars().try_fold((0, None), |(mut pen, last_glyph_index), character| -> anyhow::Result<_> {
        let glyph_index = font.glyph_index(character);
        let glyph = font.glyph(glyph_index);
        let (size, coverage_vec) = glyph.rasterize(scale);
        let coverage = coverage_vec.image(size.into()).unwrap();
        let stb_glyph_index = stb.find_glyph_index(character as u32);
        if let Some(last_glyph_index) = last_glyph_index { pen += stb.get_glyph_kern_advance(last_glyph_index, stb_glyph_index); }
        // text_line_ascent=max(glyph.ascent) => text_line_ascent-glyph.ascent >= 0
        let target = target.slice_mut((vec2(pen as f32, (text_line_ascent-glyph.ascent) as f32)*scale).try_into()
            //.with_context(||format!("{:?}",(pen, glyph.offset_y, scale)))
            ?, coverage.size)
            .with_context(||format!("{:?}",(pen, coverage.size)))?;
        //.or_print((pen, coverage.size))?;
        //offset.x+size.x <= self.size.x && offset.y+size.y <= self.size.y
        //println!("Blit {}",target.size);
        for (coverage, target) in (coverage, target).pixels() {
            assert!(*coverage<=1f32);
            let a = (coverage*(256.0-std::f32::EPSILON)) as u8;
            assert_eq!((1f32*(256.0-std::f32::EPSILON)) as u8, 255);
            *target = bgra8{b : a, g : a, r : a, a : 0xFF}; // premultiplied alpha sRGB #FFFFFF. output = (1-a)*background + foreground
            //*target = argb8{a : 0xFF, r : 0xFF, g : 0xFF, b : 0xFF}; // Tranparent background. compositor sRGB blend
        }
        Ok((pen+glyph.advance_width as i32, Some(stb_glyph_index)))
    })?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
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
    layer_surface.assign_mono(move |layer_surface, event| { match event {
            Event::Closed => {},
            Event::Configure {serial, width, height} => {
                if !(width > 0 && height > 0) {
                    layer_surface.set_size(3840/8/3, 2160/3/3); //layer_surface.set_size(3840/3, 2160/3/3); // fixme: get compositor size
                    layer_surface.ack_configure(serial);
                    surface.commit();
                    return;
                }
                let size = size2::from((width*3, height*3));
                let file = tempfile::tempfile().unwrap();
                file.set_len((size.x*size.y*4) as u64).unwrap();
                let bytes = &mut unsafe{memmap::MmapMut::map_mut(&file)}.unwrap()[..]; // ? unmap on drop
                let mut target = unsafe{std::slice::from_raw_parts_mut(bytes.as_mut_ptr() as *mut bgra8, bytes.len() / std::mem::size_of::<bgra8>())}.image(size).unwrap();
                // Unwinding here seems to crash sway soon after :/ Show partial render instead
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{ render(&mut target).unwrap_or_else(|e| println!("{}", e)) })).unwrap_or_else(|_| println!("Panic"));
                // FIXME: reuse pool+buffer
                let pool = shm.create_pool((&file as &dyn std::os::unix::io::AsRawFd).as_raw_fd(), (target.size.x*target.size.y*4) as i32);
                let buffer = pool.create_buffer(0, target.size.x as i32, target.size.y as i32, (target.stride*4) as i32, wayland_client::protocol::wl_shm::Format::Argb8888);
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
                wayland_client::protocol::wl_pointer::Event::Enter { /*surface_x, surface_y,*/ .. } => { /*println!("Pointer entered at ({}, {}).", surface_x, surface_y);*/ }
                wayland_client::protocol::wl_pointer::Event::Leave { .. } => {
                    //println!("Pointer left.");
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
