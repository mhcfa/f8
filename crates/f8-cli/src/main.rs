fn main() {
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    let exit_code = f8_cli::run();
    std::process::exit(exit_code);
}
