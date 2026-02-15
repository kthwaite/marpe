use std::path::PathBuf;

pub struct Args {
    pub root: PathBuf,
    pub tls: bool,
    pub cert: Option<PathBuf>,
    pub key: Option<PathBuf>,
    pub port: u16,
    pub syntax_theme_light: String,
    pub syntax_theme_dark: String,
    pub open: bool,
}

pub fn parse_args() -> Args {
    let mut args = std::env::args().skip(1);
    let mut root: Option<PathBuf> = None;
    let mut tls = false;
    let mut cert: Option<PathBuf> = None;
    let mut key: Option<PathBuf> = None;
    let mut port: u16 = 13181;
    let mut syntax_theme_light = "InspiredGitHub".to_string();
    let mut syntax_theme_dark = "base16-ocean.dark".to_string();
    let mut open = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--tls" => tls = true,
            "--open" => open = true,
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
            "--syntax-theme-light" => {
                if let Some(t) = args.next() {
                    syntax_theme_light = t;
                } else {
                    eprintln!("Missing theme name for --syntax-theme-light");
                    std::process::exit(1);
                }
            }
            "--syntax-theme-dark" => {
                if let Some(t) = args.next() {
                    syntax_theme_dark = t;
                } else {
                    eprintln!("Missing theme name for --syntax-theme-dark");
                    std::process::exit(1);
                }
            }
            "--help" | "-h" => {
                eprintln!("Usage: markdown-preview [OPTIONS] [DIRECTORY]");
                eprintln!();
                eprintln!("Options:");
                eprintln!("  --tls          Enable HTTPS (uses mkcert certificates)");
                eprintln!("  --open         Open the browser automatically");
                eprintln!("  --cert <PATH>  TLS certificate file (PEM)");
                eprintln!("  --key <PATH>   TLS private key file (PEM)");
                eprintln!("  --port <PORT>  Starting port (default: 13181)");
                eprintln!("  --syntax-theme-light <THEME>  Syntax theme for light mode (default: InspiredGitHub)");
                eprintln!("  --syntax-theme-dark <THEME>   Syntax theme for dark mode (default: base16-ocean.dark)");
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

    Args { root, tls, cert, key, port, syntax_theme_light, syntax_theme_dark, open }
}
