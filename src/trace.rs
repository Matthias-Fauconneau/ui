#[fehler::throws(rstack_self::Error)] pub fn rstack_self() {
	if std::env::args().nth(1).unwrap_or_default() == "rstack-self" {
		rstack_self::child()?;
		std::process::exit(0);
	}
}

pub fn trace() {
	for thread in rstack_self::trace(std::process::Command::new(std::env::current_exe().unwrap()).arg("rstack-self")).unwrap().threads().first() {
		struct Symbol<'t> {/*file: &'t str,*/ line: u32, name: &'t str};
		let mut symbols = thread.frames().iter().rev().flat_map(|frame|
			frame.symbols().iter().rev().filter_map(|sym|
				/*sym.file().map(std::path::Path::to_str).flatten().map(|file|*/ sym.line().map(|line| sym.name().map(|mut name| {
					if let Some(hash) = name.rfind("::") { name = name.split_at(hash).0; }
					/*let file = file.strip_suffix(".rs").unwrap_or(file);
					let file = file.split_at(file.rmatch_indices('/').map(|(i,_p)| i).nth(2).unwrap_or(0)+1).1;*/
					Symbol{/*file,*/line,name}
				}))/*)*/.flatten()//.flatten()
			)
		);
		for Symbol{line,name,..} in &mut symbols { if name.ends_with("::main") { println!("{}:{}", name, line); break; } }
		for Symbol{line,name,..} in symbols { println!("{}:{}", name, line); }
	}
}

cfg_if::cfg_if! { if #[cfg(feature="timeout")] {
#[fehler::throws(std::io::Error)] fn timeout_<T>(task: impl FnOnce()->T, timeout: std::time::Duration) -> T {
	let done = std::sync::atomic::AtomicBool::new(false);
	let watchdog = || {
		let start = std::time::Instant::now();
		let mut remaining = timeout;
		while !done.load(std::sync::atomic::Ordering::Acquire) {
			std::thread::park_timeout(remaining);
			let elapsed = start.elapsed();
			if elapsed >= timeout { trace(); std::process::abort() }
			println!("restarting park_timeout after {:?}", elapsed);
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
pub fn timeout<T>(task: impl FnOnce()->T) -> T { timeout_(task, std::time::Duration::from_millis(1)).unwrap() }
}}

cfg_if::cfg_if! { if #[cfg(feature="signal-hook")] {
pub fn sigint_trace() { std::thread::spawn(|| for _ in signal_hook::iterator::Signals::new(&[signal_hook::SIGINT]).unwrap().forever() { trace(); std::process::abort() }) }
}}
