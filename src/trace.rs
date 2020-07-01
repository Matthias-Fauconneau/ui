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

cfg_if::cfg_if! { if #[cfg(feature="timeout")] {
#[fehler::throws(std::io::Error)] fn timeout_<T>(task: impl FnOnce()->T, timeout: std::time::Duration, debug: impl std::fmt::Debug + std::marker::Sync) -> T {
	let done = std::sync::atomic::AtomicBool::new(false);
	let watchdog = || {
		let start = std::time::Instant::now();
		let mut remaining = timeout;
		while !done.load(std::sync::atomic::Ordering::Acquire) {
			std::thread::park_timeout(remaining);
			let elapsed = start.elapsed();
			if elapsed >= timeout { trace(); eprintln!("{:?}", debug); std::process::abort() }
			eprintln!("restarting park_timeout after {:?}", elapsed);
			remaining = timeout - elapsed;
		}
	};
	let watchdog = unsafe { std::thread::Builder::new().spawn_unchecked(watchdog)? };
	let result = task();
	done.store(true, std::sync::atomic::Ordering::Release);
	watchdog.thread().unpark();
	watchdog.join().unwrap();
	result
}
pub fn timeout<T>(debug: impl std::fmt::Debug + std::marker::Sync, task: impl FnOnce()->T) -> T { timeout_(task, std::time::Duration::from_millis(1), debug).unwrap() }
}}

#[cfg(feature="signal-hook")]
pub fn sigint_trace() { std::thread::spawn(|| for _ in signal_hook::iterator::Signals::new(&[signal_hook::SIGINT]).unwrap().forever() { trace(); std::process::abort() }); }
