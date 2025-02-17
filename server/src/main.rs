use std::fs::File;
use std::io::BufReader as StdBufReader;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_rustls::{TlsAcceptor, rustls::{self, Certificate, PrivateKey, RootCertStore, ServerConfig, AllowAnyAuthenticatedClient}};
use rustls_pemfile::{certs, rsa_private_keys};

/// Load one or more certificates from a PEM file.
fn load_certs(path: &str) -> std::io::Result<Vec<Certificate>> {
    let certfile = File::open(path)?;
    let mut reader = StdBufReader::new(certfile);
    let certs = certs(&mut reader)?
        .into_iter()
        .map(Certificate)
        .collect();
    Ok(certs)
}

/// Load a private key from a PEM file (RSA in this example).
fn load_private_key(path: &str) -> std::io::Result<PrivateKey> {
    let keyfile = File::open(path)?;
    let mut reader = StdBufReader::new(keyfile);
    let keys = rsa_private_keys(&mut reader)?;
    if keys.is_empty() {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "No private keys found"));
    }
    Ok(PrivateKey(keys[0].clone()))
}

/// Load client CA certificates into a RootCertStore.
fn load_client_ca(path: &str) -> std::io::Result<RootCertStore> {
    let mut root_cert_store = RootCertStore::empty();
    let ca_certs = load_certs(path)?;
    for cert in ca_certs {
        root_cert_store.add(&cert).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid CA certificate")
        })?;
    }
    Ok(root_cert_store)
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Load server's certificate and private key.
    let certs = load_certs("certificates/server_cert.pem")?;
    let key = load_private_key("certificates/server_key.pem")?;
    
    // Load the CA certificates to verify client certificates.
    let client_ca = load_client_ca("certificates/client_ca.pem")?;

    // Build the rustls ServerConfig with mutual TLS.
    let config = ServerConfig::builder()
        .with_safe_defaults()
        .with_client_cert_verifier(AllowAnyAuthenticatedClient::new(client_ca))
        .with_single_cert(certs, key)
        .expect("bad certificate or key");

    let tls_acceptor = TlsAcceptor::from(Arc::new(config));

    // Bind to port 443.
    let listener = TcpListener::bind("0.0.0.0:443").await?;
    let (tx, _rx) = broadcast::channel(20);

    println!("TLS server listening on port 443");

    loop {
        let (socket, addr) = listener.accept().await?;
        let tls_acceptor = tls_acceptor.clone();
        let tx = tx.clone();
        let mut rx = tx.subscribe();

        tokio::spawn(async move {
            // Perform the TLS handshake.
            let stream = match tls_acceptor.accept(socket).await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("TLS handshake failed with {}: {:?}", addr, e);
                    return;
                }
            };

            let (reader, mut writer) = tokio::io::split(stream);
            let mut reader = BufReader::new(reader);
            let mut line = String::new();

            loop {
                tokio::select! {
                    result = reader.read_line(&mut line) => {
                        if result.unwrap() == 0 {
                            break;
                        }
                        if let Err(e) = tx.send((line.clone(), addr)) {
                            eprintln!("Broadcast error: {:?}", e);
                        }
                        line.clear();
                    },
                    result = rx.recv() => {
                        let (msg, other_addr) = result.unwrap();
                        if addr != other_addr {
                            if let Err(e) = writer.write_all(msg.as_bytes()).await {
                                eprintln!("Write error: {:?}", e);
                                break;
                            }
                        }
                    }
                }
            }
        });
    }
}
