use std::env;
use std::io::{BufRead, BufReader};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::{thread, time};

pub struct RddnsProcess {
    process: Child,
    stdout: BufReader<ChildStdout>
}

/// Represents a running rddns instance.
///
/// The process will be stoped when [RddnsProcess] goes ot of scope.
impl RddnsProcess {

    /// Starts a new rddns process.
    pub fn new() -> RddnsProcess {
        let executable = env::current_exe()
            .unwrap().parent().and_then(|p| p.parent())
            .expect("The test executable should have two parent directories be available.")
            .to_path_buf().join("rddns");
        let mut process = Command::new(executable)
            .stdout(Stdio::piped())
            //.stdout(Stdio::inherit())
            .spawn()
            .expect("Spawning the rrdns process should work");

        let stdout_raw = process.stdout.take().unwrap();
        let stdout = BufReader::new(stdout_raw);

        let rddns = RddnsProcess {
            process,
            stdout
        };

        rddns.wait_for_start();

        rddns
    }

    pub fn get_url(&self) -> &str {
        "http://localhost:3000"
    }

    pub fn stdout_readln(&mut self) -> String {
        let mut buffer = String::new();
        self.stdout.read_line(&mut buffer).unwrap();
        buffer
    }

    fn stop(& mut self) {
        self.process.kill().expect("Stopping rrdns process should work.");
        self.process.wait().unwrap();
    }

    fn wait_for_start(& self) {
        let startup_time = time::Duration::from_millis(100);
        thread::sleep(startup_time);
    }
}

impl Drop for RddnsProcess {
    fn drop(&mut self) {
        self.stop();
    }
}