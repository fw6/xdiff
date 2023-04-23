fn main() {
    if atty::is(atty::Stream::Stdout) {
        println!("stdout is tty");
    } else {
        println!("stdout is not tty");
    }
}
