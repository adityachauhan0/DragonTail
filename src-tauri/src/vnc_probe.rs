use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

pub fn vnc_probe(host: &str, port: u16) -> Result<bool, String> {
    if host.is_empty() || port == 0 {
        return Err("host and port are required".into());
    }
    let target = if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };
    let mut addrs = target.to_socket_addrs().map_err(|err| err.to_string())?;
    let Some(addr) = addrs.next() else {
        return Err(format!("no address for {target}"));
    };
    match TcpStream::connect_timeout(&addr, Duration::from_secs(2)) {
        Ok(stream) => {
            drop(stream);
            Ok(true)
        }
        Err(err)
            if err.kind() == std::io::ErrorKind::ConnectionRefused
                || err.kind() == std::io::ErrorKind::TimedOut =>
        {
            Ok(false)
        }
        Err(err) => Err(err.to_string()),
    }
}
