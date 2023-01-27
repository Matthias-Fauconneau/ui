#![feature(vec_into_raw_parts)]

fn cast<T,U>(v: Box<[T]>) -> Box<[U]> {
    use std::mem::size_of;
    assert!(size_of::<U>()%size_of::<T>() == 0);
    let (ptr, len, cap) = Vec::from(v).into_raw_parts();
    unsafe { Vec::from_raw_parts(ptr as *mut U, len*size_of::<U>()/size_of::<T>(), cap*size_of::<U>()/size_of::<T>()) }.into_boxed_slice()
}
use {image::{xy, Image, rgb8, rgb8_to_10}, ui::{size, Widget, time}};

pub fn rgb10(target: &mut Image<&mut [u32]>, source: Image<&[rgb8]>) {
    let (num, den) = if source.size.x*target.size.y > source.size.y*target.size.x { (source.size.x, target.size.x) } else { (source.size.y, target.size.y) };
    let ref map = image::sRGB_to_PQ10;
    time("",|| for y in 0..std::cmp::min(source.size.y*den/num, target.size.y) {
        for x in 0..std::cmp::min(source.size.x*den/num, target.size.x) {
            target[xy{x,y}] = rgb8_to_10(map, source[xy{x: x*num/den, y: y*num/den}]);
        }
    })
}

pub struct ImageView(pub Image<Box<[rgb8]>>);
impl Widget for ImageView {
    fn size(&mut self, size: size) -> size {
        let ref source = self.0;
        let (num, den) = if source.size.x*size.y > source.size.y*size.x { (source.size.x, size.x) } else { (source.size.y, size.y) };
        xy{x: std::cmp::min(source.size.x*den/num, size.x), y: std::cmp::min(source.size.y*den/num, size.y)}
    }
    #[fehler::throws(ui::Error)] fn paint(&mut self, target: &mut ui::Target, _: ui::size, _: ui::int2) { rgb10(target, self.0.as_ref()) }
}

fn main() -> ui::Result {
    for i in 0..1<<10 {
        assert_eq!(i, image::PQ10(image::from_PQ10(i)));
        //println!("{i} {}", image::from_PQ10(i)*1024.);
    }
    let image = image_io::io::Reader::open(std::env::args().skip(1).next().unwrap_or("test.jpg".into()))?.decode()?.into_rgb8();
    ui::run("image", &mut ImageView(image::Image::new(image.dimensions().into(), cast(image.into_raw().into_boxed_slice()))))
}
