#[fehler::throws] pub fn rstack_self() { if std::env::args().nth(1).unwrap_or_default() == "rstack-self" { rstack_self::child()?; throw!("") } }

#[fehler::throws] pub fn trace_sigint() {
    rstack_self()?;
    std::thread::spawn(move || {
        for _ in signal_hook::iterator::Signals::new(&[signal_hook::SIGINT]).unwrap().forever() {
            for thread in rstack_self::trace(std::process::Command::new(std::env::current_exe().unwrap()).arg("rstack-self")).unwrap().threads() {
                println!("{}", thread.name());
                for frame in thread.frames() {
                    for sym in frame.symbols() {
                        if let (Some(path),Some(line)) = (sym.file(),sym.line()) { print!("{}:{}: ", path.display(), line); }
                        println!("{}", sym.name().unwrap_or_default());
                    }
                }
            }
            std::process::abort();
        }
    });
}
