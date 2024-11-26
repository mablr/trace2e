use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command};

pub(crate) struct WhyEnf {
    clock: u64,
    child: Child,
}

impl WhyEnf {
    pub fn new() -> std::io::Result<Self> {
        let mut child = Command::new("../../whyenf/bin/whyenf.exe")
            .args([
                "-sig",
                "../../whyenf/trace2e.sig",
                "-formula",
                "../../whyenf/trace2e.mfotl",
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()?;

        // Consume the greeter (yes it's dirty :-P)
        let mut stdout = BufReader::new(child.stdout.as_mut().unwrap()).lines();
        for _ in 0..4 {
            stdout.next().unwrap()?;
        }

        Ok(WhyEnf { clock: 0, child })
    }

    fn write_to_stdin(&mut self, input: &str) -> std::io::Result<()> {
        let stdin = self.child.stdin.as_mut().unwrap();
        stdin.write_all(input.as_bytes())
    }

    fn read_from_stdout(&mut self) -> std::io::Result<String> {
        let mut stdout =
            std::io::BufRead::lines(std::io::BufReader::new(self.child.stdout.as_mut().unwrap()));
        let mut output = String::new();
        while let Some(res) = stdout.next() {
            match res {
                Ok(str) => {
                    if str.ends_with("OK.") {
                        break;
                    } else {
                        output.push_str(&str);
                    }
                }
                Err(e) => return Err(e),
            }
        }
        Ok(output)
    }

    pub fn prompt(&mut self, predicate: &str) -> std::io::Result<String> {
        // Assuming predicate is correct
        let prompt = format!("@{} {};\n", self.clock, predicate);

        self.write_to_stdin(&prompt)?;
        self.read_from_stdout()
    }

    pub fn incr_clock(&mut self) -> std::io::Result<String> {
        // increment clock
        self.clock += 1;
        let prompt = format!("@{};\n", self.clock);

        self.write_to_stdin(&prompt)?;
        self.read_from_stdout()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whyenf_launch() {
        let whyenf = WhyEnf::new();
        assert!(whyenf.is_ok());
    }

    #[test]
    fn whyenf_prompt() -> Result<(), Box<dyn std::error::Error>> {
        let mut whyenf = WhyEnf::new()?;

        let output1 = whyenf.prompt("A(1)")?;
        assert_eq!(output1, "[Enforcer] @0 reactively commands:Cause:B(1)");

        let output2 = whyenf.prompt("A(2)")?;
        assert_eq!(output2, "[Enforcer] @0 reactively commands:Cause:B(2)");

        let output3 = whyenf.prompt("A(3)")?;
        assert_eq!(output3, "[Enforcer] @0 reactively commands:Cause:B(3)");

        Ok(())
    }

    #[test]
    fn whyenf_incr_clock() -> Result<(), Box<dyn std::error::Error>> {
        let mut whyenf = WhyEnf::new()?;

        let output1 = whyenf.incr_clock()?;
        assert_eq!(output1, "");

        Ok(())
    }
}
