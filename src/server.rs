use std::{net::SocketAddr, sync::Arc};


mod filters;

#[derive(Debug)]
struct Unauthorized;

impl warp::reject::Reject for Unauthorized {}

#[derive(Debug)]
pub struct Headers {
    pub authorization: String,
    pub user_id: String,
    pub client_name: String,
}

impl Headers {
    pub fn new(authorization: String, user_id: String, client_name: String) -> Self {
        Self {
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
