extern crate futures;
extern crate hyper;
extern crate tokio;

pub mod rddns_driver;

use rddns_driver::RddnsProcess;

#[test]
fn prints_to_console_when_run() {
    // test
    let mut rddns = RddnsProcess::new("update");

    assert!(!rddns.is_running().unwrap());
    // assert!(rddns.stdout_readln().ends_with("updating DDNS entries\n"));
}
