use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct CliCommand<'a> {
    exec: &'a str,
    args: Vec<&'a str>,
}

impl<'a> CliCommand<'a> {
    pub fn new(exec: &'a str, args: Vec<&'a str>) -> Self {
        Self {
            exec: exec,
            args: args,
        }
    }
}

impl<'a> From<CliCommand<'a>> for tokio::process::Command  {
    fn from(value: CliCommand<'a>) -> Self {
        let mut command = tokio::process::Command::new(value.exec);
        command.args(value.args);
        command
    }
}
