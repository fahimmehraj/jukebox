use std::{fmt, net::SocketAddr, sync::Arc};

mod filters;

#[derive(Debug)]
struct Unauthorized;

impl warp::reject::Reject for Unauthorized {}

pub struct Headers {
    pub client_addr: SocketAddr,
    pub user_id: String,
    pub client_name: String,
    authorization: String,
}

impl Headers {
    pub fn new(
        client_addr: SocketAddr,
        authorization: String,
        user_id: String,
        client_name: String,
    ) -> Self {
        Self {
            client_addr,
            authorization,
            user_id,
            client_name,
        }
    }

    pub fn verify(self, authorization: Arc<String>) -> Option<Self> {
        if self.authorization != *authorization {
            return None;
        }
        Some(self)
    }
}

impl fmt::Debug for Headers {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Foo")
            .field("client_addr", &self.client_addr)
            .field("user_id", &self.user_id)
            .field("client_name", &self.client_name)
            .finish()
    }
}

/// More fields coming soon
pub struct Server {
    password: String,
    address: SocketAddr,
}

impl Server {
    pub fn _new(password: String, address: SocketAddr) -> Self {
        Self { password, address }
    }

    pub async fn run(self) {
        let password = Arc::new(self.password);
        let routes = filters::routes(password);
        warp::serve(routes).run(self.address).await;
    }
}
