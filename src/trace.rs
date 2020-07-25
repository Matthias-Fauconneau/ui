#[fehler::throws(rstack_self::Error)] pub fn rstack_self() {
	if std::env::args().nth(1).unwrap_or_default() == "rstack-self" {
		rstack_self::child()?;
		std::process::exit(0);
	}
}

pub fn trace() {
	for thread in rstack_self::trace(std::process::Command::new(std::env::current_exe().unwrap()).arg("rstack-self")).unwrap().threads().first() {
		struct Symbol<'t> {line: u32, name: &'t str};
		let mut symbols = thread.frames().iter().rev().flat_map(|frame|
			frame.symbols().iter().rev().filter_map(|sym|
				sym.line().map(|line| sym.name().map(|mut name| {
					if let Some(hash) = name.rfind("::") { name = name.split_at(hash).0; }
					Symbol{line,name}
				})).flatten()
			)
		);
		for Symbol{line,name,..} in &mut symbols { if name.ends_with("::main") { eprintln!("{}:{}", name, line); break; } }
		for Symbol{line,name,..} in symbols { eprintln!("{}:{}", name, line); }
	}
}

#[cfg(feature="signal-hook")]
pub fn sigint_trace() { std::thread::spawn(|| for _ in signal_hook::iterator::Signals::new(&[signal_hook::SIGINT]).unwrap().forever() { trace(); std::process::abort() }); }
