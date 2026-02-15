use std::path::PathBuf;

pub struct Args {
    pub root: PathBuf,
    pub tls: bool,
    pub cert: Option<PathBuf>,
    pub key: Option<PathBuf>,
    pub port: u16,
}

pub fn parse_args() -> Args {
    let mut args = std::env::args().skip(1);
    let mut root: Option<PathBuf> = None;
    let mut tls = false;
    let mut cert: Option<PathBuf> = None;
    let mut key: Option<PathBuf> = None;
    let mut port: u16 = 13181;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--tls" => tls = true,
            "--cert" => cert = args.next().map(PathBuf::from),
            "--key" => key = args.next().map(PathBuf::from),
            "--port" => {
                if let Some(p) = args.next() {
                    port = p.parse().expect("Invalid port number");
                } else {
                    eprintln!("Missing port number");
                    std::process::exit(1);
                }
            }
            "--help" | "-h" => {
                eprintln!("Usage: markdown-preview [OPTIONS] [DIRECTORY]");
                eprintln!();
                eprintln!("Options:");
                eprintln!("  --tls          Enable HTTPS (uses mkcert certificates)");
                eprintln!("  --cert <PATH>  TLS certificate file (PEM)");
                eprintln!("  --key <PATH>   TLS private key file (PEM)");
                eprintln!("  --port <PORT>  Starting port (default: 13181)");
                eprintln!("  -h, --help     Show this help");
                std::process::exit(0);
            }
            other if !other.starts_with('-') => {
                root = Some(PathBuf::from(other));
            }
            other => {
                eprintln!("Unknown option: {other}");
                std::process::exit(1);
            }
        }
    }

    let root = root.unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

    Args { root, tls, cert, key, port }
}
