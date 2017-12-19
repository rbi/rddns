extern crate hyper;
extern crate futures;

mod server;

fn main() {
    let s = server::Server {};
    s.start_server(do_update);
}

fn do_update() {
    println!("would update now");
}