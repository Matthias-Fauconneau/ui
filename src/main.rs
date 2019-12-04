#![feature(type_ascription,try_blocks)]

#[allow(non_camel_case_types)] #[derive(Clone, Copy)] struct uint2 { x: u32, y : u32 }
impl From<(u32,u32)> for uint2 { fn from(o : (u32, u32)) -> Self { Self{x:o.0,y:o.1} } }
#[allow(non_camel_case_types)] type size2 = uint2;
#[allow(non_camel_case_types)] type offset2 = uint2;

struct Image<T> {
    stride : u32,
    size : size2,
    data : T,
}
trait IntoImage {
    type Image;
    fn image(self, size : size2) -> Option<Self::Image>;
}
impl<'t, T> IntoImage for &'t [T] {
    type Image = Image<&'t [T]>;
    fn image(self, size : size2) -> Option<Self::Image> {
        if self.len() == (size.x*size.y) as usize { Some(Self::Image{stride: size.x, size, data: self}) } else { None }
    }
}
impl<'t, T> IntoImage for &'t mut [T] {
    type Image = Image<&'t mut [T]>;
    fn image(self, size : size2) -> Option<Self::Image> {
        if self.len() == (size.x*size.y) as usize { Some(Self::Image{stride: size.x, size, data: self}) } else { None }
    }
}

impl<T> Image<&mut [T]> {
    fn slice_mut(&mut self, offset : offset2, size : size2) -> Option<Image<&mut[T]>> {
        if offset.x+size.x <= self.size.x && offset.y+size.y <= self.size.y { Some(Image{size, stride: self.stride, data: &mut self.data[(offset.y*self.stride+offset.x) as usize..] }) } else { None }
    }
}

trait Offset { fn offset(&self, offset : isize) -> Self; }
impl<T> Offset for *const T { fn offset(&self, offset : isize) -> Self { unsafe{(*self).offset(offset)} } }
impl<T> Offset for *mut T { fn offset(&self, offset : isize) -> Self { unsafe{(*self).offset(offset)} } }

struct Lines<T> { ptr: T, stride: isize }
impl<T:Offset> Lines<T> { fn next(&mut self) { self.ptr = self.ptr.offset(self.stride); } }

trait IntoLines {
    type Ptr : Offset;
    fn end(&self) -> Self::Ptr;
    fn lines(&mut self) -> Lines<Self::Ptr>;
    type Element : Sized;
    fn index(ptr : &Self::Ptr, x : usize) -> Self::Element;
}
impl<'t, T> IntoLines for Image<&'t [T]> {
    type Ptr = *const T;
    fn end(&self) -> Self::Ptr { unsafe{(self.data as *const [T] as Self::Ptr).offset((self.size.y*self.stride) as isize)} }
    fn lines(&mut self) -> Lines<Self::Ptr> { Lines::<Self::Ptr>{ptr: self.data as *const [T] as Self::Ptr, stride: self.stride as isize} }
    type Element = &'t T;
    fn index(ptr : &Self::Ptr, x : usize) -> Self::Element { unsafe{&*ptr.offset(x as isize)} }
}
impl<'t, T> IntoLines for Image<&'t mut [T]> {
    type Ptr = *mut T;
    fn end(&self) -> Self::Ptr { unsafe{(self.data as *const [T] as Self::Ptr).offset((self.size.y*self.stride) as isize)} }
    fn lines(&mut self) -> Lines<Self::Ptr> { Lines::<Self::Ptr>{ptr: self.data as *mut [T] as Self::Ptr, stride: self.stride as isize} }
    type Element = &'t mut T;
    fn index(ptr : &Self::Ptr, x : usize) -> Self::Element { unsafe{&mut *ptr.offset(x as isize)} }
}

struct PixelIterator<T0, T1> where Image<T0> : IntoLines, Image<T1> : IntoLines {
    width : usize,
    end: <Image<T0> as IntoLines>::Ptr,
    lines: (Lines<<Image<T0> as IntoLines>::Ptr>, Lines<<Image<T1> as IntoLines>::Ptr>), // (Lines<T::Ptr>...)
    x : usize,
}

impl<T0, T1> Iterator for PixelIterator<T0, T1> where Image<T0> : IntoLines, Image<T1> : IntoLines, <Image<T0> as IntoLines>::Ptr : Eq {
    type Item = (<Image<T0> as IntoLines>::Element, <Image<T1> as IntoLines>::Element);
    #[inline] // test mov add add test jcc (SIB) inc ~ 7
    fn next(&mut self) -> Option<Self::Item> {
        if self.x == self.width {
            self.x = 0;
            self.lines.0.next(); self.lines.1.next(); // next(self.lines)...
            if self.lines.0.ptr == self.end { None? }
        }
        let item = Some((Image::<T0>::index(&self.lines.0.ptr, self.x), Image::<T1>::index(&self.lines.1.ptr, self.x))); // self.x.index(self.lines)...
        self.x += 1;
        item
    }
}

trait IntoPixelIterator { type PixelIterator; fn pixels(&mut self) -> Self::PixelIterator; }
impl<T0, T1> IntoPixelIterator for (Image<T0>, Image<T1>) where Image<T0> : IntoLines, Image<T1> : IntoLines {
    type PixelIterator = PixelIterator<T0, T1>;
    fn pixels(&mut self) -> Self::PixelIterator {
        Self::PixelIterator{
            width : self.0.size.x as usize,
            end: self.0.end(),
            lines: (self.0.lines(), self.1.lines()), // (self.lines()...)
            x: 0
        }
    }
}

#[allow(non_camel_case_types, dead_code)] #[derive(Clone, Copy)] struct argb8 { a: u8, r : u8, g : u8, b : u8 }

fn render(target : &mut Image<&mut[argb8]>) -> anyhow::Result<()> {
    let font_data = unsafe{memmap::Mmap::map(&std::fs::File::open("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")?)}?;
    let stb = stb_truetype::FontInfo::new(&*font_data, 0).unwrap(); // FIXME: (kern|ttf-parser)/fontdue
    let text = "Hello World! ⅞";
    let (text_width, _, _) = text.chars().fold((0, 0, None), |(_, mut pen, last_glyph_index), character| {
        let glyph_index = stb.find_glyph_index(character as u32);
        if let Some(last_glyph_index) = last_glyph_index { pen += stb.get_glyph_kern_advance(last_glyph_index, glyph_index); }
        let metrics = stb.get_glyph_h_metrics(glyph_index);
        let width = (try {
            let rect = stb.get_glyph_box(glyph_index)?;
            rect.x1-rect.x0
        } : Option<_>) .unwrap_or(0);
        let x = pen + metrics.left_side_bearing + width as i32;
        (x, pen+metrics.advance_width, Some(glyph_index))
    });
    let scale = target.size.x as f32 / text_width as f32;
    println!("{:?}", (target.size.x, text_width, scale));
    let font_data = unsafe{memmap::Mmap::map(&std::fs::File::open("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf").unwrap())}.unwrap();
    let font = fontdue::Font::from_bytes(font_data).unwrap(); // hash, //path
    text.chars().fold((0., None), |(mut pen, last_glyph_index), character| {
        let glyph_index = font.glyph_index(character);
        let glyph = font.glyph(glyph_index);
        let (size, coverage_vec) = glyph.rasterize(scale);
        let coverage = coverage_vec.image(size2::from(size)).unwrap();
        if let Some(last_glyph_index) = last_glyph_index { pen += stb.get_glyph_kern_advance(last_glyph_index, glyph_index) as f32; }
        let target = target.slice_mut(offset2::from((0, 0)), coverage.size).unwrap();
        for (coverage, target) in (coverage, target).pixels() {
            *target = argb8{a : (coverage*255.99998) as u8, r : 0xFF, g : 0xFF, b : 0xFF}; // Tranparent background. compositor sRGB blend
        }
        (pen+glyph.advance_width, Some(glyph_index))
    });
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
                if !(width > 0 && height > 0) { layer_surface.set_size(3840/3, 2160/3/3); layer_surface.ack_configure(serial); surface.commit(); return; }
                let size = size2::from((width*3, height*3));
                let file = tempfile::tempfile().unwrap();
                file.set_len((size.x*size.y*4) as u64).unwrap();
                let bytes = &mut unsafe{memmap::MmapMut::map_mut(&file)}.unwrap()[..]; // ? unmap on drop
                let mut target = unsafe{std::slice::from_raw_parts_mut(bytes.as_mut_ptr() as *mut argb8, bytes.len() / std::mem::size_of::<argb8>())}.image(size).unwrap();
                render(&mut target).unwrap_or_else(|e| println!("{}", e));
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
