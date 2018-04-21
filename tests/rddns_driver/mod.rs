use std::env;
use std::io::{BufRead, BufReader, Result};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::{thread, time};
use std::path::PathBuf;

pub struct RddnsProcess {
    process: Child,
    stdout: BufReader<ChildStdout>,
}

/// Represents a running rddns instance.
///
/// The process will be stoped when [RddnsProcess] goes ot of scope.
impl RddnsProcess {
    /// Starts a new rddns process.
    ///
    /// * `command` -  The rddns sub-command that should be executed.
    pub fn new(command: &str) -> RddnsProcess {
        let executable = target_dir().join("rddns");
        let example_config = rddns_driver_src_dir().join("empty_config.toml");

        let mut process = Command::new(executable)
            .arg("-c")
            .arg(example_config)
            .arg(command)
            .stdout(Stdio::piped())
            .spawn()
            .expect("Spawning the rrdns process should work");

        let stdout_raw = process.stdout.take().unwrap();
        let stdout = BufReader::new(stdout_raw);

        let rddns = RddnsProcess {
            process,
            stdout,
        };

        rddns.wait_for_start();

        rddns
    }

    pub fn get_url(&self) -> &str {
        "http://localhost:3092"
    }

    pub fn stdout_readln(&mut self) -> String {
        let mut buffer = String::new();
        self.stdout.read_line(&mut buffer).unwrap();
        buffer
    }

    pub fn is_running(&mut self) -> Result<bool> {
        match self.process.try_wait()? {
            Some(_) => Ok(false),
            None => Ok(true)
        }
    }

    fn stop(&mut self) -> Result<()> {
        if self.is_running()? {
            self.process.kill()?
        }
        self.process.wait()?;
        Ok(())
    }

    fn wait_for_start(&self) {
        let startup_time = time::Duration::from_millis(100);
        thread::sleep(startup_time);
    }
}

impl Drop for RddnsProcess {
    fn drop(&mut self) {
        self.stop().unwrap();
    }
}

fn target_dir() -> PathBuf {
    env::current_exe()
        .unwrap().parent().and_then(|p| p.parent())
        .expect("The test executable should have two parent directories be available.")
        .to_path_buf()
}

fn base_dir() -> PathBuf {
    target_dir()
        .parent().unwrap()
        .parent().unwrap()
        .to_path_buf()
}

fn rddns_driver_src_dir() -> PathBuf {
    base_dir().join("tests").join("rddns_driver")
}