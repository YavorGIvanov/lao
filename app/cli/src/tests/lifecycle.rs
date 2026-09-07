use super::*;

#[test]
#[ignore = "uses cached Qwen/llama.cpp, installed harnesses and isolated launchd jobs; local inference only"]
fn each_harness_warms_smokes_and_restores_independently() {
    let actual = paths().unwrap();
    let model = lao_model::open(&actual.model).expect("verified cached model");
    let bin = lao_run::binary(&actual.runtime);
    let (runtime, endpoint) = lao_run::Direct::start(lao_run::Config {
        bin: &bin,
        model: &model.path,
        mode: lao_run::Mode::Light,
        working_set: model.artifact.working_set,
        context: model.artifact.context,
        threads: 2,
    })
    .expect("bounded local runtime");
    let daemon = env::var_os("LAO_TEST_DAEMON")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            env::current_exe()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("lao-daemon")
        });
    assert!(daemon.is_file(), "build lao-daemon before this test");

    for client in [ClientChoice::Codex, ClientChoice::Claude] {
        let temp = Temp::new();
        let port = free_port().unwrap();
        let (paths, mut transaction, codex, claude, _) = prepared(&temp, client, port);
        let clients = preflight_clients(client).unwrap();
        if clients.codex.is_some() {
            fs::copy(
                actual.codex.parent().unwrap().join("models_cache.json"),
                paths.codex.parent().unwrap().join("models_cache.json"),
            )
            .unwrap();
        }
        let key = temp.0.join("runtime.key");
        write_atomic(&key, endpoint.bearer().as_bytes(), 0o600).unwrap();
        let selected = Selected {
            choice: Choice {
                router: Router::Safe,
                runtime: Runtime::External,
                client,
            },
            vllm: None,
            external: Some(Adapter {
                addr: endpoint.addr().to_string().parse().unwrap(),
                key: Some(key),
            }),
        };
        fs::copy(&daemon, &paths.daemon).unwrap();
        fs::set_permissions(&paths.daemon, fs::Permissions::from_mode(0o700)).unwrap();
        write_atomic(&paths.worker_key, caller().unwrap().as_bytes(), 0o600).unwrap();
        let codex_key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let claude_key = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
        let label = format!("com.lao.single.{}.{port}", std::process::id());
        let plist = plist(
            &paths,
            &selected,
            port,
            codex_key,
            claude_key,
            &clients,
            &paths.worker_key,
        )
        .unwrap()
        .replace(LABEL, &label);
        write_atomic(&paths.plist, plist.as_bytes(), 0o600).unwrap();
        // Drop unloads only this uniquely named test job, including on assertion failure.
        let mut job = Job(Some(format!("{}/{}", domain().unwrap(), label)));
        bootstrap(&paths).unwrap();
        verify_ready(&paths, port).unwrap();
        transaction.apply().unwrap();
        let store = lao_optimize::Store::new(&paths.optimize);
        let deadline = Instant::now() + Duration::from_secs(180);
        loop {
            match store.load().unwrap() {
                Some(OptimizeState::Ready) => break,
                Some(OptimizeState::Failed) => panic!("selected client warmup failed"),
                _ => assert!(
                    Instant::now() < deadline,
                    "selected client warmup timed out"
                ),
            }
            thread::sleep(Duration::from_millis(100));
        }
        transaction.validate_installed().unwrap();
        smoke_clients(&transaction).unwrap();

        let (disabled, caller) = if client == ClientChoice::Codex {
            ("/ant/v1/messages", codex_key)
        } else {
            ("/oai/responses", claude_key)
        };
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        write!(stream, "POST {disabled} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nX-LAO-Key: {caller}\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}").unwrap();
        let mut response = Vec::new();
        stream.take(4096).read_to_end(&mut response).unwrap();
        assert!(response.is_empty(), "unselected client must be denied");

        transaction.restore().unwrap();
        job.stop().unwrap();
        store.remove().unwrap();
        transaction.discard().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while TcpStream::connect_timeout(
            &std::net::SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
            Duration::from_millis(100),
        )
        .is_ok()
        {
            assert!(Instant::now() < deadline, "test listener survived bootout");
            thread::sleep(Duration::from_millis(20));
        }
        let (managed, before, untouched) = if client == ClientChoice::Codex {
            (&paths.codex, codex, &paths.claude)
        } else {
            (&paths.claude, claude, &paths.codex)
        };
        assert_eq!(fs::read(managed).unwrap(), before);
        assert_eq!(fs::read_link(untouched).unwrap(), temp.0.join("unselected"));
        assert_eq!(store.load().unwrap(), None);
        println!(
            "{client:?}: selected-client warmup, local smoke, disabled-client denial and restoration passed"
        );
    }
    runtime.stop().unwrap();
}

struct Job(Option<String>);

impl Job {
    fn stop(&mut self) -> io::Result<()> {
        if let Some(service) = &self.0 {
            if !Command::new("/bin/launchctl")
                .args(["bootout", service])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()?
                .success()
            {
                return Err(invalid("test job bootout"));
            }
            self.0 = None;
        }
        Ok(())
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
