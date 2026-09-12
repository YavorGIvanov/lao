//! Private stdio fixture oracle and bounded runtime for the public workflow collector.
#[path = "../tests/support/tasks.rs"]
mod fixtures;
use fixtures::*;
use serde_json::{Value, json};
use std::{
    env,
    io::{self, BufRead, Write},
    path::PathBuf,
    process::{Command, Stdio},
};

fn main() {
    if run().is_err() {
        eprintln!("workflow support failed");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let tasks = workflow_tasks();
    let mut fixture: Option<(usize, Repo, Tree, Value)> = None;
    let mut runtime: Option<lao_run::Direct> = None;
    let home = PathBuf::from(env::var_os("HOME").ok_or("home unavailable")?);
    let cache = home.join("Library/Caches/lao");
    for line in io::stdin().lock().lines() {
        let line = line?;
        if line.len() > 4096 {
            return Err("request too large".into());
        }
        let request: Value = serde_json::from_str(&line)?;
        let result = match request["op"].as_str() {
            Some("prepare") => {
                drop(fixture.take());
                let index = tasks
                    .iter()
                    .position(|task| Some(task.id.as_str()) == request["task"].as_str())
                    .ok_or("unknown task")?;
                let task = &tasks[index];
                let repo = Repo::new(task);
                if !Command::new("/usr/bin/git")
                    .env_clear()
                    .env("GIT_CONFIG_NOSYSTEM", "1")
                    .env("GIT_CONFIG_GLOBAL", "/dev/null")
                    .current_dir(&repo.0)
                    .args(["add", "--", &task.file, "README.md"])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()?
                    .success()
                {
                    return Err("fixture index".into());
                }
                let before = tree(&repo.0).ok_or("fixture scope")?;
                let expected = task.expected();
                if verdict(&repo.0, task, &before, &expected) != (false, true) {
                    return Err("starting verifier".into());
                }
                let result =
                    json!({"root": repo.0, "file": task.file, "objective": task.objective});
                fixture = Some((index, repo, before, expected));
                result
            }
            Some("verify") => {
                let (index, repo, before, expected) = fixture.as_ref().ok_or("missing fixture")?;
                let (verified, scope_ok) = verdict(&repo.0, &tasks[*index], before, expected);
                json!({"verified": verified, "scope_ok": scope_ok})
            }
            Some("dispose") => {
                fixture = None;
                json!({"disposed": true})
            }
            Some("start") => {
                if runtime.is_some() {
                    return Err("runtime already loaded".into());
                }
                let model = lao_model::open(&cache.join("models"))?;
                let bin = lao_run::binary(&cache.join("runtimes"));
                let (started, endpoint) = lao_run::Direct::start(lao_run::Config {
                    bin: &bin,
                    model: &model.path,
                    mode: lao_run::Mode::Light,
                    working_set: model.artifact.working_set,
                    context: model.artifact.context,
                    threads: 2,
                })?;
                let result =
                    json!({"addr": endpoint.addr().to_string(), "bearer": endpoint.bearer()});
                runtime = Some(started);
                result
            }
            Some("stop") => {
                if let Some(started) = runtime.take() {
                    started.stop()?;
                }
                json!({"stopped": true})
            }
            _ => return Err("unknown operation".into()),
        };
        serde_json::to_writer(io::stdout(), &result)?;
        println!();
        io::stdout().flush()?;
    }
    Ok(())
}
