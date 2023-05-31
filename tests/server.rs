extern crate hyper;
extern crate futures;
extern crate tokio;

mod rddns_driver;

use hyper::Client;
use tokio::runtime::Runtime;
use rddns_driver::RddnsProcess;

#[test]
fn prints_to_console_on_request() {

    // setup
    let mut rddns = RddnsProcess::new("trigger");

    let client = Client::new();
    let uri = rddns.get_url().parse().unwrap();
    let request = client.get(uri);

    let rt = Runtime::new().unwrap();

    match rt.block_on(request) {
        Ok(response) => {
            // test
            assert!(response.status().as_u16() < 300);

            assert!(rddns.stdout_readln().ends_with("Listening on port 3092\n"));
            assert!(rddns.is_running().unwrap())
        }
        Err(err) => panic!("{}", err)
    }
}
