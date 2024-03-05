pub fn set_hook() {
    std::panic::set_hook(Box::new(panic_hook_unix));
}

fn panic_hook_unix(info: &std::panic::PanicInfo) {
    eprint!("Error");

    if let Some(msg) = info.message() {
        eprint!(": {}", msg);
    }

    if let Some(loc) = info.location() {
        eprint!(" at {}", loc);
    }

    eprintln!();
}
