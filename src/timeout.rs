#[fehler::throws(std::io::Error)] fn timeout_<T>(task: impl FnOnce()->T, time: std::time::Duration, display: impl std::fmt::Display + std::marker::Sync) -> T {
	if time.is_zero() { task() } else {
		let done = std::sync::atomic::AtomicBool::new(false);
		let watchdog = || {
			let start = std::time::Instant::now();
			let mut remaining = time;
			while !done.load(std::sync::atomic::Ordering::Acquire) {
				std::thread::park_timeout(remaining);
				let elapsed = start.elapsed();
				if elapsed >= time {
					eprintln!("{}", display);
					#[cfg(feature="trace")] crate::trace::trace();
					std::process::abort()
				}
				eprintln!("restarting park_timeout after {:?}", elapsed);
				remaining = time - elapsed;
			}
		};
		let watchdog = unsafe { std::thread::Builder::new().spawn_unchecked(watchdog)? };
		let result = task();
		done.store(true, std::sync::atomic::Ordering::Release);
		watchdog.thread().unpark();
		watchdog.join().unwrap();
		result
	}
}
//pub fn timeout<T>(debug: impl std::fmt::Debug + std::marker::Sync, task: impl FnOnce()->T) -> T { timeout_(task, std::time::Duration::from_millis(1), debug).unwrap() }
//#[track_caller] pub fn timeout<T>(task: impl FnOnce()->T) -> T { timeout_(task, std::time::Duration::from_millis(1), std::panic::Location::caller()).unwrap() }
#[track_caller] pub fn timeout<T>(time: u64, task: impl FnOnce()->T) -> T { timeout_(task, std::time::Duration::from_millis(time), std::panic::Location::caller()).unwrap() }

