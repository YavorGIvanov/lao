mod supervisor;

use std::{
    env,
    error::Error,
    fs::{self, OpenOptions},
    io::{self, Write},
};

pub fn run() -> Result<(), Box<dyn Error + Send + Sync>> {
    let _ = (lao_route::status(), lao_model::status(), lao_run::status());
    let listener = supervisor::listener("gate")?;
    if let Some(path) = env::var_os("LAO_ADOPTED_FILE") {
        let address = listener.local_addr()?.to_string();
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(mut file) => write!(file, "{address}")?,
            Err(error)
                if error.kind() == io::ErrorKind::AlreadyExists
                    && fs::read_to_string(path)? == address => {}
            Err(error) => return Err(error.into()),
        }
    }
    lao_gate::closed(listener)
}
