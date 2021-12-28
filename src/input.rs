use crate::prelude::*;
#[allow(non_upper_case_globals)] static usb_hid_usage_table: std::lazy::SyncLazy<Vec<char>> = std::lazy::SyncLazy::new(|| [
	&['\0','⎋','1','2','3','4','5','6','7','8','9','0','-','=','⌫','\t','q','w','e','r','t','y','u','i','o','p','{','}','\n','⌃','a','s','d','f','g','h','j','k','l',';','\'','`','⇧','\\','z','x','c','v','b','n','m',',','.','/','⇧','\0','⎇',' ','⇪'],
	&(1..=10).map(|i| (0xF700u32+i).try_into().unwrap()).collect::<Vec<_>>()[..], &['\0'; 20], &['\u{F70B}','\u{F70C}'], &['\0'; 8],
	&['⎙','⎄',' ','⇤','↑','⇞','←','→','⇥','↓','⇟','⎀','⌦','\u{F701}','🔇','🕩','🕪','⏻','=','±','⏯','🔎',',','\0','\0','¥','⌘']].concat());
#[allow(non_upper_case_globals)] const usb_hid_buttons: [u32; 2] = [272, 111];

use ::xy::xy;
use crate::widget::{EventContext, Event, ModifiersState};
use wayland_client::{Dispatch, ConnectionHandle, QueueHandle as Queue, DataInit, WEnum, protocol::{
	wl_seat::{self as seat, WlSeat as Seat},
	wl_keyboard::{self as keyboard, WlKeyboard as Keyboard},
	wl_pointer::{self as pointer, WlPointer as Pointer}
}};
use super::State;

impl Dispatch<Seat> for State {
    type UserData = ();
    fn event(&mut self, seat: &Seat, event: seat::Event, _: &Self::UserData, cx: &mut ConnectionHandle, queue: &Queue<Self>, _: &mut DataInit<'_>) {
        match event {
			seat::Event::Capabilities{capabilities: WEnum::Value(capabilities)} => {
				if capabilities.contains(seat::Capability::Keyboard) { seat.get_keyboard(cx, queue, ()).unwrap(); }
				if capabilities.contains(seat::Capability::Pointer) { seat.get_pointer(cx, queue, ()).unwrap(); }
			},
			_ => {}
        }
    }
}

impl State { #[throws] pub fn key(&mut self, key: char) -> bool {
	let Self{size, modifiers_state, widget, ..} = self;
	if widget.event(*size, &EventContext{modifiers_state: *modifiers_state, pointer: None}, &Event::Key{key})? { self.need_update = true; true }
	else if key == '⎋' { self.running=false; false }
	else { false }
}}

impl Dispatch<Keyboard> for State {
    type UserData = (); //let mut repeat : Option<std::rc::Rc<std::cell::Cell<_>>> = None;
    fn event(&mut self, _: &Keyboard, event: keyboard::Event, _: &Self::UserData, _: &mut ConnectionHandle, _: &Queue<Self>, _: &mut DataInit<'_>) {
        use keyboard::{Event::*, KeyState};
		match event {
			Keymap{..} => {},
			Enter{ /*keysyms,*/ .. } => {},
			Leave{ .. } => {}
			Key{state: WEnum::Value(state), key, .. } => {
				let key = *usb_hid_usage_table.get(key as usize).unwrap_or_else(|| panic!("{:x}", key));
				match state {
					KeyState::Released => { /*if repeat.as_ref().filter(|r| r.get()==key ).is_some() { repeat = None }*/ },
					KeyState::Pressed => {
						self.key(key).unwrap();
						/*repeat = {
							let repeat = std::rc::Rc::new(std::cell::Cell::new(key));
							let from_monotonic_millis = |t| {
								let now = {let rustix::time::Timespec{tv_sec, tv_nsec} = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic); tv_sec * 1000 + tv_nsec / 1000_000};
								std::time::Instant::now() - std::time::Duration::from_millis((now - t as i64) as u64)
							};
							use futures_lite::StreamExt;
							self.streams.push(
								async_io::Timer::interval_at(from_monotonic_millis(time)+std::time::Duration::from_millis(150), std::time::Duration::from_millis(33))
								.filter_map({
									let repeat = std::rc::Rc::downgrade(&repeat);
									// stops and autodrops from streams when weak link fails to upgrade (repeat cell dropped)
									move |_| { repeat.upgrade().map(|x| {let key = x.get(); (box move |w| { w.key(key)?; w.draw() }) as Box::<dyn FnOnce(&mut App<'t,W>)->Result<()>>}) }
								})
								.fuse()
								.boxed_local()
							);
							Some(repeat)
						};*/
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
					const NUM_LOCK : u32 = 0b10000000000000000000;
					assert_eq!([mods_depressed&!(SHIFT|CAPS|CTRL|ALT|LOGO|NUM_LOCK), mods_latched, mods_locked&!CAPS, locked_group], [0,0,0,0]);
					self.modifiers_state = ModifiersState {
							shift: mods_depressed&SHIFT != 0,
							ctrl: mods_depressed&CTRL != 0,
							logo: mods_depressed&LOGO != 0,
							alt: mods_depressed&ALT != 0,
					}
			},
			RepeatInfo {..} => {},
			_ => unreachable!()
		}
	}
}

impl Dispatch<Pointer> for State {
    type UserData = ();
    fn event(&mut self, pointer: &Pointer, event: pointer::Event, _: &Self::UserData, _: &mut ConnectionHandle, _: &Queue<Self>, _: &mut DataInit<'_>) {
		let event_context = EventContext{modifiers_state: self.modifiers_state, pointer: Some(pointer)};
		match event {
			pointer::Event::Motion{surface_x: x, surface_y: y, ..} => {
				self.cursor_position = self.scale as f32*xy{x: x as _, y: y as _};
				if self.widget.event(self.size, &event_context, &Event::Motion{position: self.cursor_position, mouse_buttons: self.mouse_buttons}).unwrap() { self.need_update = true; }
			},
			pointer::Event::Button{button, state: WEnum::Value(state), ..} => {
				let button = usb_hid_buttons.iter().position(|&b| b == button).unwrap_or_else(|| panic!("{:x}", button)) as u8;
				if state == pointer::ButtonState::Pressed { self.mouse_buttons |= 1<<button; } else { self.mouse_buttons &= !(1<<button); }
				if self.widget.event(self.size, &event_context, &Event::Button{button, state, position: self.cursor_position}).unwrap() { self.need_update = true; }
			},
			pointer::Event::Axis {axis: WEnum::Value(pointer::Axis::VerticalScroll), value, ..} => {
				if self.widget.event(self.size, &event_context, &Event::Scroll(value as f32)).unwrap() { self.need_update = true; }
			},
			_ => {},
		}
	}
}