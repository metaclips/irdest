use clap::{App, Arg};
use directories::ProjectDirs;
use ratman_client::{Identity, RatmanIpc, Receive_Type};
use serde::{Deserialize, Serialize};
use std::{
    fs::{create_dir, File},
    io::{stdin, stdout, Read, Write},
    os::unix::prelude::AsRawFd,
    path::PathBuf,
};

pub fn build_cli() -> App<'static, 'static> {
    App::new("ratcat")
        .about("Client management program not unlike cat, but for ratman")
        .version(env!("CARGO_PKG_VERSION"))
        .after_help("ratcat(1) stores current address information in $XDG_CONFIG_DIR/ratcat/config\n\nThis is ALPHA level software and will include bugs and cause crashes.  If you encounter a reproducible issue, please report it in our issue tracker (https://git.irde.st/we/irdest) or our mailing list: https://lists.irde.st/archives/list/community@lists.irde.st")
        .max_term_width(120)
        .arg(
            Arg::with_name("RECIPIENT")
                .takes_value(true)
                .required_unless_one(&["REGISTER", "RECEIVE"])
                .help("Specify the message recipient address.  Not required when calling `--register` or `--recv`")
        )
        .arg(
            Arg::with_name("MESSAGE")
                .takes_value(true)
                .help("Provide a message to send across the network.  If no <MESSAGE> is provided ratcat will read a message from standard input!")
        )
        .arg(
            Arg::with_name("SENDER")
                .long("sender")
                .short("s")
                .takes_value(true)
                .help("Specify the sender address instead of using the default one stored in $XDG_CONFIG_HOME")
        )
        .arg(
            Arg::with_name("REGISTER")
                .long("register")
                .help("Register a new address on the network with the Ratman daemon")
        )
        .arg(
            Arg::with_name("RECEIVE")
                .long("recv")
                .help("Set your computer to receive data via ratcat.")
        )
        .arg(
            Arg::with_name("RECV_COUNT")
                .long("count")
                .takes_value(true)
                .help("Specify the number of messages that `--recv` should wait for.  Default value is to wait forever.")
        )
        .arg(
            Arg::with_name("API_BIND")
                .takes_value(true)
                .long("bind")
                .short("b")
                .help("Specify the API socket bind address")
                .default_value("127.0.0.1:9020"),
        )
}

#[derive(Serialize, Deserialize)]
struct Config {
    addr: Identity,
    token: Vec<u8>,
}

async fn register(path: PathBuf, bind: &str) -> Result<(), Box<dyn std::error::Error>> {
    let ipc = RatmanIpc::connect(bind, None).await?;
    let mut f = File::create(path.join("config"))?;

    let cfg = Config {
        addr: ipc.address(),
        token: vec![],
    };
    let cfg_str = serde_json::to_string_pretty(&cfg)?;
    f.write_all(cfg_str.as_bytes())?;
    eprintln!("Registered address: {}", ipc.address());
    Ok(())
}

async fn connect_ipc(cfg: &Config, bind: &str) -> Result<RatmanIpc, Box<dyn std::error::Error>> {
    Ok(RatmanIpc::connect(bind, Some(cfg.addr)).await?)
}

/// Returns the number of messages sent
async fn send(
    ipc: &RatmanIpc,
    recp: &str,
    msg: Option<&str>,
) -> Result<usize, Box<dyn std::error::Error>> {
    let recp = Identity::from_string(&recp.to_string());

    // Either turn the provided string message into a byte array or read from stdin
    match msg.map(|s| s.as_bytes().to_vec()) {
        Some(msg) => {
            ipc.send_to(recp, msg).await?;
            Ok(1)
        }
        None => {
            let mut ctr = 0;
            let mut stdin = stdin();
            let mut vec = vec![0; 1024];
            while let Ok(_) = stdin.read_exact(&mut vec) {
                ipc.send_to(recp, vec.clone()).await?;
                ctr += 1;
            }

            Ok(ctr)
        }
    }
}

async fn handle_receives(ipc: &RatmanIpc, num: usize) -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = stdout();
    let is_tty = nix::unistd::isatty(stdout.as_raw_fd()).unwrap_or(false);

    for _ in if num == 0 { 0..std::usize::MAX } else { 0..num } {
        let (tt, mut msg) = match ipc.next().await {
            Some(msg) => msg,
            None => break,
        };

        if tt == Receive_Type::FLOOD {
            continue;
        }

        let payload: Vec<_> = msg.take_payload();
        if is_tty {
            println!(
                "{}",
                String::from_utf8(payload).unwrap_or("<Unprintable data>".to_string())
            );
        } else {
            stdout.write_all(&payload)?;
        }
    }

    Ok(())
}

#[async_std::main]
async fn main() {
    let app = build_cli();
    let m = app.clone().get_matches();

    //// Setup the application config directory
    let dirs = ProjectDirs::from("org", "irdest", "ratcat")
        .expect("Failed to initialise project directories for this platform!");
    let cfg_dir = PathBuf::from(dirs.config_dir());
    let _ = create_dir(&cfg_dir);

    let num: usize = match m.value_of("RECV_COUNT").map(|c| c.parse().ok()) {
        Some(Some(num)) => num,
        Some(None) => {
            eprintln!("Failed to parse `--count` as a number!");
            std::process::exit(2);
        }
        None => 0,
    };

    //// To register is a bit special because we terminate afterwards
    let api_addr = m.value_of("API_BIND").unwrap();
    if m.is_present("REGISTER") {
        match register(cfg_dir, api_addr).await {
            Ok(_) => {
                eprintln!("Registered a new address!  You may now run `ratcat` to send data");
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("An error occured during registration: {:?}", e);
                std::process::exit(1);
            }
        }
    }

    //// Open the configuration a previous us left behind :)
    let mut cfg = match File::open(cfg_dir.join("config")) {
        Ok(mut f) => {
            let mut s = String::new();
            f.read_to_string(&mut s).unwrap();
            serde_json::from_str::<Config>(s.as_str()).expect("failed to parse config!")
        }
        Err(_) => {
            eprintln!("No configuration found!  Run `ratcat --register` first!");
            std::process::exit(2);
        }
    };

    //// Check if a sender address was provided via CLI options
    if let Some(addr) = m.value_of("SENDER") {
        cfg.addr = Identity::from_string(&addr.to_owned());
    }

    //// We always need to connect to the IPC backend with our address
    eprintln!("Connecting to IPC backend...");
    let ipc = match connect_ipc(&cfg, api_addr).await {
        Ok(ipc) => ipc,
        Err(e) => {
            eprintln!("Failed to connect to Ratman daemon: {}", e);
            std::process::exit(1);
        }
    };

    //// If we were given a recipient we send try to send some data
    if let Some(recipient) = m.value_of("RECIPIENT") {
        let message = m.value_of("MESSAGE");
        match send(&ipc, recipient, message).await {
            Ok(num) => num,
            Err(e) => {
                eprintln!("An error occured during sending: {:?}", e);
                std::process::exit(1);
            }
        };
    }

    //// If we were given the --recv flag we try to receive some data
    if m.is_present("RECEIVE") {
        match handle_receives(&ipc, num).await {
            Ok(()) => {}
            Err(e) => {
                eprintln!("Failed to receive data: {}", e);
                std::process::exit(1);
            }
        }
    }
}
