use std::convert::TryFrom;
use std::lazy::SyncLazy;
#[allow(non_upper_case_globals)] static usb_hid_usage_table: SyncLazy<Vec<char>> = SyncLazy::new(|| [
	&['\0','⎋','1','2','3','4','5','6','7','8','9','0','-','=','⌫','\t','q','w','e','r','t','y','u','i','o','p','{','}','\n','⌃','a','s','d','f','g','h','j','k','l',';','\'','`','⇧','\\','z','x','c','v','b','n','m',',','.','/','⇧','\0','⎇',' ','⇪'],
	&(1..=10).map(|i| char::try_from(0xF700u32+i).unwrap()).collect::<Vec<_>>()[..], &['\0'; 20], &['\u{F70B}','\u{F70C}'], &['\0'; 8],
	&['⎙','⎄',' ','⇤','↑','⇞','←','→','⇥','↓','⇟','⎀','⌦','\u{F701}','🔇','🕩','🕪','⏻','=','±','⏯','🔎',',','\0','\0','¥','⌘']].concat());
#[allow(non_upper_case_globals)] const usb_hid_buttons: [u32; 2] = [272, 111];

use error::Result;
use client_toolkit::{seat::{SeatData, pointer::ThemeManager}, get_surface_scale_factor, reexports::client::{Attached, protocol::{wl_seat::WlSeat as Seat, wl_keyboard as keyboard, wl_pointer as pointer}}};
use {::xy::xy, crate::{app::App, widget::{Widget, EventContext, Event, ModifiersState}}};

pub fn seat<'t, W:Widget>(theme_manager: &ThemeManager, seat: &Attached<Seat>, seat_data: &SeatData) {
	if seat_data.has_keyboard {
		let mut repeat : Option<std::rc::Rc<std::cell::Cell<_>>> = None;
		seat.get_keyboard().quick_assign(move |_, event, mut app| {
			let app = unsafe{std::mem::transmute::<&mut App<&mut dyn Widget>,&mut App<'t,W>>(app.get::<App<&mut dyn Widget>>().unwrap())};
			use keyboard::{Event::*, KeyState};
			match event {
				Keymap {..} => {},
				Enter { /*keysyms,*/ .. } => {},
				Leave { .. } => {}
				Key {state, key, time, .. } => {
					let key = *usb_hid_usage_table.get(key as usize).unwrap_or_else(|| panic!("{:x}", key));
					match state {
						KeyState::Released => { if repeat.as_ref().filter(|r| r.get()==key ).is_some() { repeat = None } },
						KeyState::Pressed => {
							app.key(key).unwrap();
							repeat = {
								let repeat = std::rc::Rc::new(std::cell::Cell::new(key));
								let from_monotonic_millis = |t| {
									let now = {let rsix::time::Timespec{tv_sec, tv_nsec} = rsix::time::clock_gettime(rsix::time::ClockId::Monotonic); tv_sec * 1000 + tv_nsec / 1000_000};
									std::time::Instant::now() - std::time::Duration::from_millis((now - t as i64) as u64)
								};
								use futures_lite::StreamExt;
								app.streams.push(
									async_io::Timer::interval_at(from_monotonic_millis(time)+std::time::Duration::from_millis(150), std::time::Duration::from_millis(33))
									.filter_map({
										let repeat = std::rc::Rc::downgrade(&repeat);
										// stops and autodrops from streams when weak link fails to upgrade (repeat cell dropped)
										move |_| { repeat.upgrade().map(|x| {let key = x.get(); (box move |app| { app.key(key)?; app.draw() }) as Box::<dyn FnOnce(&mut App<'t,_>)->Result<()>>}) }
									})
									.fuse()
									.boxed_local()
								);
								Some(repeat)
							};
						},
						_ => unreachable!(),
					}
				},
				Modifiers {mods_depressed, mods_latched, mods_locked, group: locked_group, ..} => {
						const SHIFT : u32 = 0b001;
						const CAPS : u32 = 0b010;
						const CTRL : u32 = 0b100;
						const ALT : u32 = 0b1000;
						const LOGO : u32 = 0b1000000;
						assert_eq!([mods_depressed&!(SHIFT|CAPS|CTRL|ALT|LOGO), mods_latched, mods_locked&!CAPS, locked_group], [0,0,0,0]);
						app.modifiers_state = ModifiersState {
								shift: mods_depressed&SHIFT != 0,
								ctrl: mods_depressed&CTRL != 0,
								alt: mods_depressed&ALT != 0,
								logo: mods_depressed&LOGO != 0,
								caps_lock: false, num_lock: false
						}
				},
				RepeatInfo {..} => {},
				_ => unreachable!()
			}
		});
	}
	if seat_data.has_pointer {
		let (mut position, mut mouse_buttons) = num::zero();
		theme_manager.theme_pointer_with_impl(&seat, move |event, mut pointer, mut app| {
			let app = unsafe{std::mem::transmute::<&mut App<&mut dyn Widget>,&mut App<'t,W>>(app.get::<App<&mut dyn Widget>>().unwrap())};
			let event_context = EventContext{modifiers_state: app.modifiers_state, pointer: Some(&mut pointer)};
			match event {
				pointer::Event::Motion{surface_x, surface_y, ..} => {
					position = {let p = get_surface_scale_factor(&app.surface) as f64*xy{x: surface_x, y: surface_y}; xy{x: p.x as u32, y: p.y as u32}};
					if app.widget.event(app.size, &event_context, &Event::Motion{position, mouse_buttons}).unwrap() { app.need_update = true; }
				},
				pointer::Event::Button{button, state, ..} => {
					let button = usb_hid_buttons.iter().position(|&b| b == button).unwrap_or_else(|| panic!("{:x}", button)) as u8;
					if state == pointer::ButtonState::Pressed { mouse_buttons |= 1<<button; } else { mouse_buttons &= !(1<<button); }
					if app.widget.event(app.size, &event_context, &Event::Button{button, state, position}).unwrap() { app.need_update = true; }
				},
				pointer::Event::Axis {axis: pointer::Axis::VerticalScroll, value, ..} => {
					if app.widget.event(app.size, &event_context, &Event::Scroll(value as f32)).unwrap() { app.need_update = true; }
				},
				_ => {},
			}
		});
	}
}
