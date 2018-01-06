extern crate tokio_core;
extern crate hyper;
extern crate futures;

mod rddns_driver;

use tokio_core::reactor::Core;
use hyper::Client;
use futures::future::Future;
use rddns_driver::RddnsProcess;

#[test]
fn writes_prints_to_console_on_request() {
    // setup
    let mut core = Core::new().unwrap();
    let client = Client::new(&core.handle());
    let mut rddns = RddnsProcess::new();

    // test
    let uri = rddns.get_url().parse().unwrap();
    let work = client.get(uri).map(|result| {
        assert!(result.status().as_u16() < 300);
    });
    core.run(work).unwrap();

    assert_eq!(rddns.stdout_readln(), "updating DDNS entries\n");
}