use std::io;
use std::io::{ErrorKind};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use stunclient::StunClient;
use tokio::net::UdpSocket;
use crate::config::IpAddressStun;

lazy_static!(
    static ref LOCAL_IPV4: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0);
    static ref LOCAL_IPV6: SocketAddr = SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0);
);

pub async  fn resolve_stun(
    config: &IpAddressStun
) -> Option<IpAddr> {

    let ip;
    if config.ipv6 {
        ip = get_ipv6(config.stun_server.clone()).await;
    } else {
        ip = get_ipv4(config.stun_server.clone()).await;
    }

    match ip {
        Ok(addr) => {
            Some(addr.ip())
        }
        Err(err) => {
            warn!("Failed to resolve IP Address using STUN Server. {:?}", err);
            None
        }
    }
}

async fn get_ipv6(stun_server: String) -> Result<SocketAddr, io::Error> {
    get(LOCAL_IPV6.clone(), stun_server, |x| x.is_ipv6()).await
}

async fn get_ipv4(stun_server: String) -> Result<SocketAddr, io::Error> {
    get(LOCAL_IPV4.clone(), stun_server, |x| x.is_ipv4()).await
}

async fn get<P>(local_addr: SocketAddr, stun_server: String, filter: P) -> Result<SocketAddr, io::Error> where P: FnMut(&SocketAddr) -> bool {
    if let Some(addr) = stun_server.to_socket_addrs()?.filter(filter).next() {
        let udp = UdpSocket::bind(&local_addr).await?;

        let client = StunClient::new(addr);
        client.query_external_address_async(&udp).await.map_err(|err| {
            io::Error::new(ErrorKind::Other, err)
        })
    } else {
        Err(io::Error::new(ErrorKind::Other, "The STUN Server does not support the ip protocol!"))
    }
}