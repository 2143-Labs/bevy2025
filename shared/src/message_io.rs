//! Dummy implementation of message-io for web builds

pub mod network {
    #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
    pub struct Endpoint(pub u32);

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
}

pub mod node {
    use super::network::{Endpoint, NetEvent, Transport};
    use core::net::SocketAddr;

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

    pub trait ToSocketAddrs {
        fn to_socket_addrs(&self) -> Result<Vec<SocketAddr>, String>;
    }

    impl NetworkController {
        pub fn listen(&self, _transport: Transport, _addr: impl ToSocketAddrs) -> Result<(Endpoint, SocketAddr), String> {
            todo!()
        }
        pub fn connect(&self, _transport: Transport, _addr: impl ToSocketAddrs) -> Result<(Endpoint, SocketAddr), String> {
            todo!()
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
    impl ToSocketAddrs for (&str, u16) {
        fn to_socket_addrs(&self) -> Result<Vec<SocketAddr>, String> {
            let addr_str = format!("{}:{}", self.0, self.1);
            match addr_str.parse::<SocketAddr>() {
                Ok(addr) => Ok(vec![addr]),
                Err(_) => Err("Invalid socket address".to_string()),
            }
        }
    }

    impl<T> NodeListener<T> {
        pub fn for_each<F>(&self, mut f: F)
        where
            F: FnMut(NodeEvent<'_, T>),
        {
            todo!()
        }
    }
}

