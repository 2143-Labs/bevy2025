//! Dummy implementation of message-io for web builds

use dashmap::DashMap;
static REGISTERED_ENDPOINTS: crate::Lazy<DashMap<network::Endpoint, core::net::SocketAddr>> = 
    crate::Lazy::new(|| DashMap::new());

pub mod network {
    use super::REGISTERED_ENDPOINTS;
    use core::net::SocketAddr;

    #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
    pub struct Endpoint(pub u32);

    impl core::fmt::Display for Endpoint {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(f, "Endpoint({})", self.0)
        }
    }

    #[derive(Clone, Debug)]
    pub enum NetEvent<'a> {
        Connected(Endpoint, &'a str),
        Accepted(Endpoint, u32),
        Message(Endpoint, &'a [u8]),
        Disconnected(Endpoint),
    }

    #[derive(Clone, Debug, Copy)]
    pub enum Transport {
        Udp,
    }

    impl Endpoint {
        pub fn addr(&self) -> SocketAddr {
            REGISTERED_ENDPOINTS
                .get(&self)
                .map(|entry| *entry.value())
                .unwrap()
        }
    }
}

pub mod node {
    use super::network::{Endpoint, NetEvent, Transport};
    use core::net::SocketAddr;
    use super::REGISTERED_ENDPOINTS;

    #[derive(Clone, Debug)]
    pub enum NodeEvent<'a, T> {
        Network(NetEvent<'a>),
        Signal(T),
    }

    #[derive(Clone, Debug)]
    pub struct NodeHandler<T> {
        _marker: std::marker::PhantomData<T>,
        network_controller: NetworkController,
    }

    #[derive(Clone, Debug)]
    pub struct NodeListener<T> {
        _marker: std::marker::PhantomData<T>,
        network_controller: NetworkController,
    }

    #[derive(Clone, Debug)]
    pub struct NetworkController {
        pub network_id: u32,
    }

    pub enum SendStatus {
        Whatever,
    }

    impl<T> NodeHandler<T> {
        pub fn network(&self) -> &NetworkController {
            &self.network_controller
        }
    }

    impl<T> NodeListener<T> {
        pub fn network(&self) -> &NetworkController {
            &self.network_controller
        }
    }

    pub trait ToSocketAddrs {
        fn to_socket_addrs(&self) -> Result<Vec<SocketAddr>, String>;
    }

    impl NetworkController {
        pub fn listen(
            &self,
            _transport: Transport,
            _addr: impl ToSocketAddrs,
        ) -> Result<(Endpoint, SocketAddr), String> {
            let id = Endpoint(rand::random_range(0..=u32::MAX));
            let sa = _addr.to_socket_addrs()?.get(0).cloned().unwrap();
            REGISTERED_ENDPOINTS.insert(id, sa);

            //TODO
            Ok((id, sa))
        }
        pub fn connect(
            &self,
            _transport: Transport,
            _addr: impl ToSocketAddrs,
        ) -> Result<(Endpoint, SocketAddr), String> {
            let id = Endpoint(rand::random_range(0..=u32::MAX));
            let sa = _addr.to_socket_addrs()?.get(0).cloned().unwrap();
            REGISTERED_ENDPOINTS.insert(id, sa);
            //TODO

            Ok((id, sa))
        }
        pub fn send(&self, _endpoint: Endpoint, _data: &[u8]) -> SendStatus {
            SendStatus::Whatever
        }
    }

    pub fn split<T>() -> (NodeHandler<T>, NodeListener<T>) {
        let network_controller = NetworkController { network_id: 0 };
        (
            NodeHandler {
                _marker: std::marker::PhantomData,
                network_controller: network_controller.clone(),
            },
            NodeListener {
                _marker: std::marker::PhantomData,
                network_controller: network_controller.clone(),
            },
        )
    }

    // GPT GENERATED: CHECK TODO
    impl<T: AsRef<str>> ToSocketAddrs for (T, u16) {
        fn to_socket_addrs(&self) -> Result<Vec<SocketAddr>, String> {
            let addr_str = format!("{}:{}", self.0.as_ref(), self.1);
            match addr_str.parse::<SocketAddr>() {
                Ok(addr) => Ok(vec![addr]),
                Err(_) => Err("Invalid socket address".to_string()),
            }
        }
    }

    impl<T: ToSocketAddrs> ToSocketAddrs for &T {
        fn to_socket_addrs(&self) -> Result<Vec<SocketAddr>, String> {
            (*self).to_socket_addrs()
        }
    }

    impl<T> NodeListener<T> {
        pub fn for_each<F>(&self, mut _f: F)
        where
            F: FnMut(NodeEvent<'_, T>),
        {
            todo!()
        }

        pub fn register_web_event_listener<F>(&self, mut _f: F)
        where
            F: FnMut(NodeEvent<'_, T>),
        {
            crate::info!("Registering web event listener");
        }
    }
}
