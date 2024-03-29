use std::io;
use std::io::ErrorKind;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use stunclient::StunClient;
use std::net::UdpSocket;
use crate::config::{AddressType, IpAddressStun};

lazy_static!(
    static ref LOCAL_IPV4: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0);
    static ref LOCAL_IPV6: SocketAddr = SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0);
);

pub fn resolve_stun(
    config: &IpAddressStun
) -> Option<IpAddr> {

    let ip = match config.address_type {
        AddressType::IPV4 => {
            get_ipv4(config.stun_server.clone())
        },
        AddressType::IPV6 => {
            get_ipv6(config.stun_server.clone())
        }
    };

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

fn get_ipv6(stun_server: String) -> Result<SocketAddr, io::Error> {
    get(LOCAL_IPV6.clone(), stun_server, |x| x.is_ipv6())
}

fn get_ipv4(stun_server: String) -> Result<SocketAddr, io::Error> {
    get(LOCAL_IPV4.clone(), stun_server, |x| x.is_ipv4())
}

fn get<P>(local_addr: SocketAddr, stun_server: String, filter: P) -> Result<SocketAddr, io::Error> where P: FnMut(&SocketAddr) -> bool {
    if let Some(addr) = stun_server.to_socket_addrs()?.filter(filter).next() {
        let udp = UdpSocket::bind(&local_addr)?;

        let client = StunClient::new(addr);
        client.query_external_address(&udp).map_err(|err| {
            io::Error::new(ErrorKind::Other, err)
        })
    } else {
        Err(io::Error::new(ErrorKind::Other, "The STUN Server does not support the ip protocol!"))
    }
}
