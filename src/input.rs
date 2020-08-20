use std::convert::TryFrom;
use std::lazy::SyncLazy;
#[allow(non_upper_case_globals)] static usb_hid_usage_table: SyncLazy<Vec<char>> = SyncLazy::new(|| [
	&['\0','⎋','1','2','3','4','5','6','7','8','9','0','-','=','⌫','\t','q','w','e','r','t','y','u','i','o','p','{','}','\n','⌃','a','s','d','f','g','h','j','k','l',';','\'','`','⇧','\\','z','x','c','v','b','n','m',',','.','/','⇧','\0','⎇',' ','⇪'],
	&(1..=10).map(|i| char::try_from(0xF700u32+i).unwrap()).collect::<Vec<_>>()[..], &['\0'; 20], &['\u{F70B}','\u{F70C}'], &['\0'; 8],
	&['⎙','⌃',' ','⇤','↑','⇞','←','→','⇥','↓','⇟','⎀','⌦','\u{F701}','🔇','🕩','🕪','⏻','=','±','⏯','🔎',',','\0','\0','¥','⌘']].concat());

//use {core::{error::{throws, Error, Result}, num::Zero}, ::xy::{xy, size}, image::bgra8, crate::widget::{Widget, Target, Event, ModifiersState}};
use std::{rc::Rc, cell::Cell};
use futures::{FutureExt, stream::{unfold, StreamExt}};
use client_toolkit::{seat::SeatData, reexports::client::{Attached, protocol::{wl_seat::WlSeat as Seat, wl_keyboard as keyboard, wl_pointer as pointer}}};
use crate::{app::App, widget::{Widget, ModifiersState}};

pub fn seat<'t, W:Widget>(seat: &Attached<Seat>, seat_data: &SeatData) {
    if seat_data.has_keyboard {
        let mut repeat : Option<Rc<Cell<_>>> = None;
        seat.get_keyboard().quick_assign(move |_, event, mut app| {
            //let app = app.get::<App<'t>>().unwrap();
            let app = unsafe{std::mem::transmute::<&mut App<&mut dyn Widget>,&mut App<'t,W>>(app.get::<App<&mut dyn Widget>>().unwrap())};
            use keyboard::{Event::*, KeyState};
            match event {
                Keymap {..} => {},
                Enter { /*keysyms,*/ .. } => {},
                Leave { .. } => {}
                Key {state, key, .. } => {
                    let key = *usb_hid_usage_table.get(key as usize).unwrap_or_else(|| panic!("{:x}", key));
                    match state {
                        KeyState::Released => if repeat.as_ref().filter(|r| r.get()==key ).is_some() { repeat = None },
                        KeyState::Pressed => {
                            app.key(key);
                            if let Some(repeat) = repeat.as_mut() { // Update existing repeat cell
                                repeat.set(key); // Note: This keeps the same timer on key repeat change. No delay! Nice!
                            } else { // New repeat timer (registers in the reactor on first poll)
                                repeat = {
                                    let repeat = Rc::new(Cell::new(key));
                                    app.streams.push(
                                        unfold(std::time::Instant::now()+std::time::Duration::from_millis(150), {
                                            let repeat = Rc::downgrade(&repeat);
                                            move |last| {
                                                let next = last+std::time::Duration::from_millis(67);
                                                use async_io::Timer;
                                                Timer::at(next).map({
                                                    let repeat = repeat.clone();
                                                    // stops and autodrops from streams when weak link fails to upgrade (repeat cell dropped)
                                                    move |_| { repeat.upgrade().map(|x| ({let key = x.get(); (box move |app| { app.key(key); }) as Box::<dyn Fn(&mut App<'t,_>)>}, next) ) }
                                                })
                                            }
                                        }).boxed_local()
                                    );
                                    Some(repeat)
                                }
                            }
                        },
                        _ => unreachable!(),
                    }
                },
                Modifiers {mods_depressed, mods_latched, mods_locked, group: locked_group, ..} => {
                    const SHIFT : u32 = 0b001;
                    const CAPS : u32 = 0b010;
                    const CTRL : u32 = 0b100;
                    const LOGO : u32 = 0b1000000;
                    assert_eq!([mods_depressed&!(SHIFT|CAPS|CTRL|LOGO), mods_latched, mods_locked&!CAPS, locked_group], [0,0,0,0]);
                    app.modifiers_state = ModifiersState {
                        shift: mods_depressed&SHIFT != 0,
                        ctrl: mods_depressed&CTRL != 0,
                        logo: mods_depressed&LOGO != 0,
                        ..Default::default()
                    }
                },
                RepeatInfo {..} => {},
                _ => unreachable!()
            }
        });
    }
    if seat_data.has_pointer {
        seat.get_pointer().quick_assign(|_, event, mut app| {
						let App{display, ..} = unsafe{std::mem::transmute::<&mut App<&mut dyn Widget>,&mut App<'t,W>>(app.get::<App<&mut dyn Widget>>().unwrap())};
            match event {
                pointer::Event::Leave{..} => *display = None,
                pointer::Event::Motion{/*surface_x, surface_y,*/..} => {},
                pointer::Event::Button{/*button, state,*/..} => {},
                _ => {},
            }
        });
    }
}
